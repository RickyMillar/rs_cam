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

## 10. Predictions on HEAD `0fad0599d`

Written 2026-11-13, before the code of (a). Sections 1 and 5 were measured before 8c; this section
measures again on HEAD. "Measured" is a run of a throwaway prototype on HEAD; "reasoned" is not
run. The prototype: the shape-test door of section 5 ported to HEAD (8b: `of_dressup` takes the
material; 8c: the plunge feed is the op plunge rate), with one rule added: the door does not run
on a rest-driven surface pass (`entry_probe_leave` and the rest stock both set), where the split's
`RestEntryRamp` owns the descent (Q3: the rest arm). A second switch put the op plunge rate on the
tab descent (the tab feed rule). Every prototype file was restored from byte copies
(`sha256sum -c` OK); the probes were deleted.

The code takes the clip re-entries from a typed list of the clip walk, not from the shape test.
Under Ramp and Helix the two select the same descents: the pipeline converted every vertical
descent before the clips, a tab descent follows a fed move, and a fallback plunge of the pipeline
is not a clip re-entry and stays (section 5.3, idx 12). So the prototype numbers are the
predictions for the code.

### 10.1 Wanaka idx 17 (measured)

| | HEAD | After (a) |
|---|---|---|
| Moves | 1750 | 1840 |
| `EntryPlunge` at 1582 | 42 | 0 |
| `EntryApproach` at 1582 | 19 | 59 |
| `EntryRamp` at 3165 | 511 | 603 |
| `ClearingCut` (3165 / 2500) | 969 / 14 | 969 / 14 |
| `Linking`, `Retract`, `ObstacleClearance` | 132, 62, 1 | 132, 62, 1 |
| Cycle time (`compute_cycle_time`, `link_kinematics`) | 710.99 s | 740.54 s (+29.55 s, +4.16 %) |
| End-of-op height map (cell 0.25 mm, disc r 3, fresh top 25.5) | | max difference 0.0 mm, 0 cells |
| Obstacle hits (`check_toolpath`, `JoinStart::Unknown`) after the step | | 0 |
| Obstacle hits on the final toolpath (after the split and the K1 backstop clip) | | 0 (reasoned: the backstop changes no move when every move keeps out; the code measures it) |

8c moved the HEAD baseline of section 5.1 (720.00 s, the air lead at 791): the air lead now runs
at the op plunge rate 1582.

What each of the 42 re-entries becomes (paired in order with the HEAD plunges):

- 37: a rapid to the material top plus the contact clearance, an air lead (`EntryApproach`, 1582),
  then 2 `EntryRamp` legs at 3165 (3 degrees), out along the following cut and back.
- 2 (the first level, Y 274.15 and 120.81): 2 `EntryRamp` legs only. The material top is the fresh
  top 25.5, so the ramp starts at the rapid floor Z 26.0 and needs no air lead.
- 1 (HEAD move 1603, into the cut-short tab ramp at Y 322.19): an air lead and a 9-leg lap
  ladder that ends at the level Z 1.25 at Y 327.25. The run to the tab is 5.06 mm, shorter than one
  fold, so the ramp laps.
- 1 (HEAD move 1616, onto the tab top at Y 120.81): an air lead and a 6-leg lap ladder that ends on
  the tab top Z 2.0 (the bite is 0.5 mm, section 3.1).
- 1 (HEAD move 1739, onto the tab top at Y 120.81): an air lead only, straight down to Z 2.0 at
  1582. This is the one re-entry that is not a ramp: the descent is air only (section 3.1), so
  `apply_entry` emits the air lead and no ramp. (Section 5 counted 3 that do not become fold ramps:
  the 2 first-level ramps with no air lead, which are ramps, and this one.)

### 10.2 The other wanaka operator ops (measured, expected none)

Idx 0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13: the move lists are identical, byte for byte. Idx 1, 2,
14, 15 are Planner contracts (3D Rough, the rest 6 mm): `effective_entry_style` gives None, so the
step does not run (reasoned; not run, they take minutes). Idx 16 (Scallop, style None): no step
(reasoned); in the tracked `nz_wanaka_21.toml` it measured 463469 moves before and after.

### 10.3 Op golden (measured)

Beside the known 46 rows (`opgolden_known_46_rows.txt`, identical at HEAD), 30 rows of 5 arms move.
Each is an obstacle re-entry at the W4 obstacle strip (X 2 to 6) of the op fixtures that becomes a
ramp of the fixture's rule angle (generic softwood, 20 degrees, 8b) folded along the following
arc. The ramp runs at F500: the fixture states no ramp feed, so the entry rule gives the plunge
feed (`OperationConfig::entry_feed_rate`: "`None` uses the plunge feed"; the emitter takes
`min(descent feed, op plunge rate)`, and the clip descent already runs at the op plunge rate).

| Arm | Rows (old -> new) |
|---|---|
| `pocket/each_face_flipped` | `span_count` 129 -> 131, `spans_hash`. No move moves (`gcode_hash` and `moves_hash` stay); the shape test put two descents in an entry span. Two readings, not told apart before the code: (1) they are clip re-entries that `apply_entry` gives back unchanged, so the typed list moves the arm in the same way; (2) they are pipeline fallback plunges after a rapid, which the shape test selects and the typed list does not, so the arm does not move. Either reading is inside this prediction |
| `pocket/extend_open` | `cutting_length_mm` 1365.236 -> 1372.931, `linear_count` 73 -> 95, `move_count` 113 -> 135, `span_count` 55 -> 57, `moves_hash`, `spans_hash`, `gcode_hash` |
| `profile/each_face` | `cutting_length_mm` 615.056 -> 624.194, `linear_count` 40 -> 66, `move_count` 82 -> 108, `span_count` 61 -> 63, `moves_hash`, `spans_hash`, `gcode_hash` |
| `profile/fresh` | `cutting_length_mm` 451.778 -> 456.587, `linear_count` 20 -> 33, `move_count` 41 -> 54, `span_count` 31 -> 32, `moves_hash`, `spans_hash`, `gcode_hash` |
| `profile/rest` | the same numbers as `profile/fresh`, with its own hashes |

The tab feed rule alone moves no golden row (measured).

### 10.4 FM1 (measured): 1 cell, against the expectation of 0

`the_sim_subset_is_the_committed_csv_fm1` moves one cell, 5 fields, all from (a) (the tab feed
rule alone moves none; HEAD passes):

| Cell | Field | Old -> new |
|---|---|---|
| Profile / EndMill / 6.0000 / mdf | `depth_peak_mm` | 4.5000 -> 0.2851 |
| | `depth_cap_mm` | "" -> 17.7397 |
| | `depth_cap_factor` | "" -> 35.3224 |
| | `depth_detail` | Reported -> "" |
| | `diagnostic_ids_after_sim` | `load.depth.reported` -> `load.depth.within` |

Six Profile sim cells change their moves (55 -> 84 twice, 69 -> 92 twice, 125 -> 140 twice; the
boundary clip of the 40 mm square re-enters, and each re-entry becomes a ramp); only this cell
changes a recorded column. Reasoned: the depth gate read the straight 4.5 mm re-entry plunge as the
peak depth; after (a) the peak is a ramp sample. The 1000 calculator cells do not move (measured).
e1 re-blesses `matrix_2026-09-23_sim.csv` for this cell and names it.

### 10.5 Tracked fixtures with a clip and a Ramp or Helix style (measured unless marked)

Each file was loaded read-only; every enabled op of a Dressup contract was generated.

| File | Op | Result |
|---|---|---|
| `tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml` and `planning/fixtures/rivmap100/rivmap100.toml` (the same op) | Face 5 (Ramp, boundary) | moves: 154 clip descents at F500. 77 land on the start of a pipeline ramp: they become a rapid to the contact clearance and an air lead (no new ramp). 77 land on a cut: they become a rapid, an air lead and 2 `EntryRamp` legs at F500 (no ramp feed stated: the entry rule gives the plunge feed). 1256 -> 1410 moves. The pipeline ramps of this op run at F2200 and F2540: feed optimization is on (`feed_optimization = true`, `feed_max_rate` 3000) and lifts them after the entry step; the step runs after the dressups, so its ramps keep the feed of the entry rule (a departure the lead can rule on: feed optimization is a dressup step and does not run again) |
| `rs_cam_viz/tests/fixtures/stale_cards/face_and_rest_rough.toml` | Face 5 | 28 descents change only their intent (`EntryPlunge` -> `EntryApproach`); positions and feeds stay, so the G-code does not move |
| `tests/fixtures/testpiece_mini_2026-10-20_c3808eb0/testpiece_mini.toml` (and the planning copy) | every Dressup op, the rest Pockets after a simulation | no change (no clip re-entry) |
| `planning/rivmap_block_job/nz_wanaka_21.toml` | idx 17 | moves, as 10.1 (1461 -> 1568 moves before the split) |
| `tests/fixtures/t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml` | op 16 (Scallop, Ramp, rest) | not measured: the fixture's mesh is absent ("Operation requires a 3D mesh"). Reasoned: no change, the rest arm owns it |
| `tests/fixtures/test_job.toml` | Ramp Finish 8 | not measured: it refuses at HEAD (no `spindle_rpm`) |
| `planning/rivmap_block_job/nz_south.toml` | | does not load at HEAD; not measured |

### 10.6 Tests that read a clip descent (measured)

These 30 binaries pass with the same result under the prototype as at HEAD (only the known
`every_op_conforms_w4` check 6 red): `boundary_reentry_plunge_rate_g_boundaryplunge`,
`boundary_clip_escape_f1`, `skipped_boundary_offset_f8`, `isoclip_link_rapid_g_isocliprapid`,
`isoclip_entry_ramp_g_isoclipentry`, `entry_descent_profile_b2`,
`transform_provenance_fingerprints`, `a_manual_retract_rises_over_the_clamp_k1`,
`a_lift_rises_to_the_plane_only_over_a_clamp_k3`,
`an_obstacle_stays_on_the_table_in_a_flipped_setup_k1`, `the_machine_head_guards_the_checks_h1`,
`capability_link_moves_safety`, `a_snippet_join_rises_to_the_plane_b18`,
`tabs_sit_on_straight_sides_p5`, `tabs_bridge_width_and_height_p5b`,
`the_tab_layout_is_the_cut_layout_p4_6`, `one_entry_rule_two_emitters_p29_8`,
`an_air_lead_is_an_approach_not_a_plunge_k4`, `a_sample_states_its_cut_kind_p29_3`,
`dressup_span_invariants`, `every_op_conforms_w4`,
`a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut`,
`every_op_declares_its_cuts_p29_3`, `the_exemption_list_only_shrinks_p29_3`,
`every_limit_judges_every_declared_cut_p29_7`, `plunge_guard_p3`,
`the_floor_reaches_the_motion_g_bottompin`, `profile_on_cuts_on_the_line_p6_3`,
`a_lead_curls_away_from_the_part_p6_0c`, `ramp_contained_in_region_g_rampcontain`. Core `--lib`:
2799 pass, 1 red (the known `the_envelope_meets_its_time_budget`).

The tests that call a clip directly (`geometry::boundary` lib tests, `compute::obstacle` lib
tests, `boundary_reentry_plunge_rate_g_boundaryplunge`, `boundary_clip_escape_f1`,
`a_lift_rises_to_the_plane_only_over_a_clamp_k3`) keep the plunge: no entry step runs there. Their
calls change shape (the typed clip result), not their claims (reasoned).

### 10.7 The tab descent under the style None (measured)

Wanaka idx 17 under None: the 16 tab descents run at 1582 (the op plunge rate) instead of 2500
(the cut feed 3165 under the Z axis cap); 77 `EntryPlunge` at 1582 in all, none at 2500; cycle
time 687.80 s -> 687.84 s. Under Ramp the rule moves nothing (the move list is byte-identical to
the one of (a) alone). Every tracked file with tabs (the wanaka 21 copies, `nz_south.toml`) cuts
its tabbed Profile under Ramp, so no tracked fixture moves (reasoned from the files; the dressup
unit tests of `apply_tabs` change their call). No golden row and no FM1 cell moves (measured).

### 10.8 The check of (c) (reasoned)

At HEAD no check reads a plunge entry against the vertical rate: on idx 17 the "plunge entry" rows
of `cut_checks` give `no_number` (`not_modelled`) for every limit but the spindle power (measured).
The check judges the stated plunge feed against `VerticalRateLimit::bound(stated, stated rpm)`, as
8d judges an entry at its stated feed over its stated RPM. It uses the existing `LimitId::PlungeRate`
("the plunge rate of the operation on a descent"); its role changes from
`NotACutLimit(MachineOrRpm)` to a value check. One row per declared cut:

- the plunge entry: the value is the stated plunge feed; on idx 17, 1582 against 1347.47
  (`AxialChip`: 0.0635 x 10610 x 2), `Exceeds`, margin -17.4 %;
- a cut with no vertical feed (the body, the first slot): `NotApplicable(NoVerticalFeed)`;
- a ramp and a helix: the vertical part of the stated entry feed (the feed x the sine of the slope)
  against the bound of that vertical rate; `VerticalRateLimit` bounds the vertical part of a G6
  ramp (`feeds/CLAUDE.md`). On idx 17 the ramp is 3165 x sin 3 deg = 165.6 mm/min, `Within`;
- a peck (the drill group): the drill feed against the bound (Step 9c bounds it).

`limits::scope` reads the cut group, and the plunge, the ramp and the helix are all in the Entry
group, so one scope cell covers the three; the role of each row comes from its engagement. If the
code needs a new scope cell with a row, or a new `Geometry` reason, e3 stops for a ruling and e1
and e2 stand alone.

No feed moves, so no golden row and no FM1 cell move (FM1 reads no `cut_checks`). The MCP
`get_safe_envelope` reply gains these rows (an added row, no key changes). The exemption pins do not
move for (c): `limits::scope` judges `PlungeRate` on every group, and the row `P28-27` already names
`PlungeRate` for the side after the simulation.

### 10.9 The exemption row and the stale texts

The commit of (a) closes the row `U-TabReplunge`: its cause (the clip re-entry under a ramp or
helix style) is fixed, and a straight plunge that stays (the style None) is the stated entry, which
(c) judges. `U-TabReplunge` is the one row with no file and needs a ruling, so the pins of
`the_exemption_list_only_shrinks_p29_3` go from `ROWS` 38, `MAX_NEEDS_RULING` 27,
`ROWS_WITH_A_FILE` 37 to 37, 26, 37 (`PHYSICAL` 8 and `RULED` 3 stay). The texts of section 0 are
rewritten in the same commit; the `entry_rule.rs:431-433` line no longer exists on HEAD (8c removed
it).

## 11. Judgement of e1 against section 10

Written 2026-11-13 after the code of (a) (`e1.patch`: the typed re-entry list of the clip walk,
`Reentering`, the entry step `enter_clip_reentries`, the plunge feed in `ResolvedEntry`, the row
`U-TabReplunge` closed). Section 10 stays as it was written. **The judgement finds two mismatches,
so the work stopped after e1**: the tab feed rule and the check (c) are not built, and e1 waits for
a ruling.

### 11.1 Predicted against measured

Each e1 run printed the number of clip re-entries that the walk recorded (a temporary print,
removed).

| Item | Section 10 | e1 (measured) | Judgement |
|---|---|---|---|
| 10.1 wanaka idx 17 | 1840 moves, 0 `EntryPlunge`, 740.54 s | byte-identical to the prototype (re-entries not counted on this file; the tracked copy records 35) | match |
| 10.2 wanaka idx 0, 3 to 13 | identical | identical, 0 re-entries each | match |
| 10.3 op golden | 30 rows on 5 arms beside the known 46 | the known 46 only. The 5 arms record 0 re-entries. Seven other arms record re-entries and do not move: `horizontal_finish/regions`, `spiral_finish/regions`, `radial_finish/regions`, `scallop/regions`, `ramp_finish/regions` (style None), `scallop/regions_rest`, `steep_shallow/rest_regions` (style None or the rest arm) | **mismatch** |
| 10.4 FM1 | 1 sim cell (Profile / EndMill / mdf) | both tests pass, 0 cells; every FM1 sim cell records 0 re-entries | **mismatch** (toward the lead's expected 0) |
| 10.5 rivmap100 Face 5 (both copies) | 1256 -> 1410 moves | byte-identical to the prototype; 154 re-entries (the boundary arm of the list) | match |
| 10.5 viz `face_and_rest_rough` Face 5 | 28 intents change | byte-identical to the prototype; 28 re-entries | match |
| 10.5 `nz_wanaka_21.toml` idx 17 | moves | byte-identical to the prototype; 35 re-entries | match |
| 10.5 `nz_wanaka_21.toml` idx 16 (Scallop, None) | no change | identical to HEAD; 152 re-entries stay plunges (style None) | match |
| 10.6 sentries | same as HEAD | the 11 binaries run on e1 pass (list in 11.4); core `--lib` 2799 pass, the known timing red | match for those run |

### 11.2 The cause of the two mismatches (measured)

The prototype selected "an `EntryPlunge` after a rapid"; e1 selects the descents that a clip
emitted. Section 10 said the two select the same descents under Ramp and Helix. That is false:

- The 5 golden arms and the 6 FM1 Profile cells have no clip re-entry (0 recorded). Their
  descents are the pipeline's own entries. In `profile/fresh` (G-code dump, HEAD and e1 equal) each
  level starts at X 9.503 Y 4.864, a point on the corner arc of the profile ring: level 1 is a
  rapid to Z 0.5 and a straight `G1 Z-2.000 F500`; each later level is an air lead to Z -1.446 and
  a straight plunge to the level. So under the Ramp style the pipeline's ramp falls back to a
  plunge there (an air lead, then the fallback plunge), and the prototype ran the same entry pass
  again after the later dressups (arc fit and the others), where the ramp folds along the arc and
  succeeds.
- It is not the lead-in: the dump holds no lead arc (the first `G2` is the ring's corner, centre
  (10, 8), radius 3.175).
- The two plunges of wanaka idx 12 (X 404.0, X 401.43) are the same kind: the walk records 0
  re-entries there, and they are pipeline fallbacks on row pieces that the territory pre-clip cut
  short (section 5.3).

Why the pipeline ramp falls back at the ring start of these fixtures and succeeds after the later
dressups is not traced.

### 11.3 Findings for the lead

- **F6.** The same entry pass gives two answers on one descent: in the pipeline the ramp falls
  back to a plunge; after the later dressups it folds along the arc. e1 does not change this. The
  prototype's shape test would have turned those plunges into ramps (30 golden rows, 1 FM1 cell).
- **F7.** The pipeline ramps of rivmap100 Face 5 run at F2200 and F2540 because feed optimization
  (on for that op) lifts them after the entry step; the ramps of the step after the clips run at
  the entry rule's F500. Feed optimization does not run again after the clips.
- **Q7.** Is the typed list (only the descents a clip emits) the wanted scope, or must a pipeline
  fallback plunge also get a second try after the dressups?
- **Q8.** Who owns the fallback plunge of F6: the entry pass (a fold that reads the final cut), or
  a ruling that a fallback plunge is the stated plunge and (c) judges it?

### 11.4 What ran on e1, and what did not

Ran on the e1 tree: workspace clippy with the heavy, research and test-support features (clean);
`cargo fmt --all -- --check` (clean); the op golden (the known 46 only); FM1 (2 pass); core
`--lib` (2799 pass, the known timing red); the sentries `the_exemption_list_only_shrinks_p29_3`,
`one_entry_rule_two_emitters_p29_8`, `every_limit_judges_every_declared_cut_p29_7`,
`every_op_declares_its_cuts_p29_3`, `boundary_reentry_plunge_rate_g_boundaryplunge`,
`boundary_clip_escape_f1`, `a_lift_rises_to_the_plane_only_over_a_clamp_k3`,
`a_manual_retract_rises_over_the_clamp_k1`, `the_machine_head_guards_the_checks_h1`,
`transform_provenance_fingerprints`, `an_air_lead_is_an_approach_not_a_plunge_k4` (all pass);
`the_envelope_meets_its_time_budget` alone three times: 299, 298, 304 ms (HEAD: 322, 319,
317 ms; e1 does not touch the envelope). The tests that read the two moved fixtures:
`the_step_records_are_stored_and_bounded_p13b` (core) and seven viz binaries
(`the_envelope_plot_shows_only_what_limits_the_cut_p5c`, `every_limit_is_drawn_and_named_p5d`,
`p17_visual_snapshots` (3 ignored), `the_chart_kit_draws_the_envelope_p17`,
`the_step_records_reach_every_surface_p13b`, `the_nomogram_readout_abstains_g_hoverbound`,
`the_sim_chart_seeks_on_a_dot_p17`): all pass. `folder_maps_name_real_files_f140` passes. Viz
`--lib` (`--test-threads 2`, no GUI running; it holds the `stale_cards` controller tests): 706
pass, 0 fail.

Not run or not done: the other sentries of 10.6, `rs_cam_mcp`, `rs_cam_cli`, the K4 tests
beyond the one above; the `CLAUDE.md` lines (`dressup/`, `geometry/`, `compute/`; `session/`
names the new file) and the `P29-8_DESIGN.md` texts of section 0; a test of one op with both clips and a
re-entry carried from the boundary clip through the obstacle clip; the obstacle hits on the final
toolpath (the K1 backstop now leaves its own re-entries as plunges, `PlungesStay::ObstacleBackstop`).

## 12. Lead rulings on section 11 (2026-11-13)

- **Q7.** The typed list is the scope. Ruling (a) of section 9 names the descents that a clip
  emits. The shape test of the prototype selected more than that, so the two mismatches are
  defects of the prediction, not of e1. The FM1 result (0 cells) is the result the lead expected.
- **Q8 / F6.** No second entry pass runs after the dressups. One entry pass that gives two answers
  on one descent is a defect of the pipeline. A trace on `profile/fresh` comes first; the ruling
  follows the trace. Until then the check (c) is the only judge of a fallback plunge.
- **`U-TabReplunge` stays open in e1.** Straight plunges still ship with no judge: the style None,
  the K1 backstop and the pipeline fallback. The row text names that cause. The pins stay at 38,
  27, 37. The commit of (c) closes the row.
- **F7.** The ramps of the step run at the feed of the entry rule (F500 on rivmap100 Face 5); the
  pipeline ramps run at the lifted feed. The step is on the slow side. Logged, no change.
- **Before the commit of e1:**
  1. the obstacle hits on the final toolpath of idx 17 are measured, not reasoned;
  2. the lead counts the moves of idx 17 on e1 against section 10.1;
  3. a Helix re-entry at a clip edge is measured against the boundary and the grown obstacle;
  4. one test carries a boundary re-entry through the obstacle clip and the K3 start rapid;
  5. the `rs_cam_mcp` and `rs_cam_cli` tests run.
- **To the clean-up pass:** `geometry/boundary.rs` imports `feeds::entry_rule`;
  `apply_dressups_with_entry` is a second door into the dressup pipeline.
- **The operator file.** The corrected boxes of setup "3 Front" are in
  `P29-8e_OBSTACLE_FRAME.md` section 5. `nz_wanaka_21_operator.toml`, `nz_wanaka_21.toml` and
  `derive_wanaka21.py` hold the same error. The lead edits none of them without the operator.
  F-OF1 (one obstacle frame on every face) goes to the P27 brief.

## 11 (addendum). Rulings Q9 and Q10 and the measurements before the commit of e1

Written 2026-11-13 after section 12. "Measured" is a throwaway probe on the e1 tree (deleted).

### Q9: a clip re-entry under the style Helix is the fold ramp

Measured before the ruling: rivmap100 Face 5 (test fixture) under Helix gave 94 helix entries of
the step, all centred on the left boundary edge (centre X -5.46 to -4.77); 1699 `EntryHelix`
moves lay outside the boundary, by up to 2.0 mm (the helix radius 2.0). No guard holds a helix of
the step: the helix containment (`entry_containment_regions`) is `Some` only for a 2D Adaptive.

Lead ruling Q9: a clip re-entry is on the clip edge by construction, a site with no room for a
helix; it takes the 8a rule of a helix with no room, the ramp at the op's own angle, folded along
the kept cut. Built as one door, `ResolvedEntry::clip_reentry` (`feeds/entry_rule.rs`), which
gives `ClipReentryEntry::Ramp` for Ramp and Helix and `StatedPlunge` for None; the step holds no
style rule.

Measured after the ruling:

| Case | Result |
|---|---|
| rivmap100 Face 5 under Helix | 0 fed moves outside the boundary; 0 obstacle hits; 99 converted sites, all folds, the worst point 0.0 mm off the kept cut |
| The 22 plunges that stay in that run | each is a clip re-entry onto a piece of a pipeline helix that the boundary clip kept as a point: the next move is the retract, so the fold has no run and the descent stays a plunge |
| wanaka idx 17 under Helix | 0 obstacle hits, 0 collisions; 43 converted sites, all folds, worst 0.0 mm. The 83 plunges at X 377.7 of the run before the ruling are gone (they came with the step's helices); 1 plunge stays: an air lead to Z 3.42 and a fed descent onto the tab top at Z 2.0 (no run on the tab top) |

The run of Face 5 under Helix on HEAD was skipped: a `git worktree` probe needs a build.

### Q10: a clip re-entry of a surface-riding op stays the straight plunge

With a surface probe, `emit_ramp` (`dressup/entry_descent.rs`) does not fold: it walks straight
legs along the direction of the next move, each up to `ENTRY_CLEARANCE / tan(angle) / 2` long
(19.1 mm at 3 degrees, 2.75 mm at 20 degrees), and `clip_polyline_to_floor` clips them in Z only.
No clip runs after the step. Measured: the step reached this branch once (rivmap100 Scallop on
fresh stock under Helix, one recorded site, 0.0 mm off the cut on that site).

Lead ruling Q10: the step converts only where the ramp folds along the kept cut. Built in
`session/compute/clip_entry.rs`: `ReentryOwner::SurfaceProbe` (an op that names
`entry_probe_leave` on fresh stock) gives `PlungesStay::SurfaceProbe`; the rest case keeps
`RestRamp` / `PlungesStay::RestEntryRamp`. The step builds its context with no surface probe, and
`enter_clip_reentries` holds a `debug_assert` that the probe is absent, so the step reaches only
the fold branch of `emit_ramp`. The fold walks only along the kept cut (`fold_ramp_points`,
`extend_fold_path` laps the run or goes round the ring); when it falls short it returns no points
and the emitter plunges after the air lead.

Measured after the ruling: the recorded site of the rivmap100 Scallop on fresh stock under Helix
is again the straight plunge (`EntryPlunge` F240 at (-1.25, 0, 12)); under Ramp the run is
identical to the run before the ruling (it records no site).

### The other items of section 12

- The final toolpath of wanaka operator idx 17 on e1 (after the split and the K1 backstop): 0
  obstacle hits, 0 collisions (`ProjectSession::collision_check`, which runs `check_toolpath`).
- The move dump of idx 17 on e1 is `scratchpad/p29_8e/e1_17.csv` (the format of `head_17.csv`);
  it is byte-identical to the list of section 10.1.
- Two lib tests in `compute/obstacle.rs`:
  `a_boundary_reentry_is_carried_through_the_obstacle_clip_p29_8e` (one boundary re-entry carried
  through the obstacle clip and its K3 start rapid; the index moves up by one and names the same
  descent; the step converts exactly that descent, onto the kept cut) and
  `a_helix_op_reenters_a_boundary_by_the_fold_ramp_q9` (no `EntryHelix` of the step, every fed
  move inside the boundary). Each went red on its defect (the shift removed; a helix at the
  re-entry). A recorded site that is not a vertical descent is copied as it is:
  `dressup::apply_entry_at` converts a move only when `sites.selects(i)` and `is_plunge` both hold.
- The row `U-TabReplunge` stays open (pins 38, 27, 37) with the cause that remains: a straight
  plunge that ships with no judge of its vertical rate (the style None, the K1 backstop re-entry,
  the pipeline fallback plunge, and now the surface-riding re-entry of Q10).
- The section 10 results do not move: idx 17, rivmap100 Face 5 and the viz Face 5 are
  byte-identical to their section 10 lists.

### Log

- Open: a fold ramp for a surface-riding clip re-entry (a fold under the probe's Z clip is a new
  emitter path with no measurement; Q10).

## 13. Predictions for the check (c), before the code

Written 2026-11-13 on the tree of e1 and e2, before the code of e3. "Measured" is a throwaway probe
(deleted) that computes, for each op of `nz_wanaka_21_operator.toml` (loaded read-only), the row
the check will write: the stated entry feed of each declared cut (the entry rule,
`ConfiguredEntry::planned_feed_mm_min`; for a ramp and a helix its vertical part: the ramp feed x
sin(angle), the helix feed x pitch / sqrt((2 pi r)^2 + pitch^2), the angle, radius and pitch of
`ResolvedEntry`; a drill its op feed) against `VerticalRateLimit::bound(value, stated rpm)`.

### 13.1 The design

- `LimitId::PlungeRate` ("the plunge rate of the operation on a descent", the existing member) is
  the limit. Its role changes from `NotACutLimit(MachineOrRpm)` to `ValueCheck(VerticalRate)`.
  `limits::scope` judges it on every cut group (Body, Entry, Cleanup, Link, Drill); a cut with no
  vertical feed gives `NotApplicable(Geometry::NoVerticalFeed)`, the existing reason. No new scope
  cell with a row and no new `Geometry` reason, so the work goes on (section 10.8).
- `gate_of(PlungeRate)` stays empty; the row `P28-27` already names `PlungeRate` for the side after
  the simulation (claim 1 of `every_limit_judges_every_declared_cut_p29_7`).
- One row of `SafeEnvelope::cut_checks` per declared cut, unit mm/min, a warning row: no feed
  moves, nothing refuses an export.
- Every straight plunge that ships is the plunge entry cut (`Entry(Plunge)`): the stated plunge
  of the style None (`Always`), and under a ramp or helix style the fallback plunge, the K1
  backstop re-entry and the surface-riding re-entry of Q10 (`MayOccur`, row `U-EntryFallback`).
  The check judges that cut in every case, so it closes the row `U-TabReplunge`: the pins of
  `the_exemption_list_only_shrinks_p29_3` go from 38, 27, 37 to 37, 26, 37.

### 13.2 The operator file (measured)

| Op | Rows that the check writes | `Exceeds` |
|---|---|---|
| 0 Face back to T1 (Ramp, 18000 rpm) | plunge 2162, ramp 337.41, helix 189.93: each the candidate | none |
| 1 Back bulk rough 18 mm (3D Rough, Helix) | plunge 2162, ramp 337.46, helix 379.43, peck 2162 | none |
| 2 Back rest 6 mm (3D Rough, Ramp) | plunge 2285, ramp 239.23, helix 795.98, peck 2285 | none |
| 3 Pin drill, 4 Drill light holes | drill 1000 | none |
| 5 River channel (Project Curve, None, 10610 rpm) | plunge 1347 (the bound 1347.47: within, margin 0.0 %) | none |
| 6, 7 Lake pockets (None) | plunge 240, ramp 12.56, helix 61.53 | none |
| 8 ANTENNA pocket (Ramp) | plunge 240, ramp 65.84, helix 322.54 | none |
| 9 V rivers and lakes, 10 Day strip slit, 13 Engrave labels (None) | plunge 381, 762, 762 | none |
| 11 Face clamp pads (None) | plunge 2162, ramp 113.15, helix 63.69 | none |
| 12 Face front to T (Ramp) | plunge 2162, ramp 337.41, helix 189.93 | none |
| 14, 15 Front roughs (3D Rough, Ramp) | plunge 2162 / 2285, ramp 337.46 / 239.23, helix 379.43 / 795.98, peck as the plunge | none |
| 16 Front scallop finish (None) | plunge 240, ramp 12.56, helix 61.53 | none |
| **17 Outline profile with tabs (Ramp, 10610 rpm)** | **plunge 1582 against 1347.47 (`AxialChip`: G6 axial chip 0.0635 x 10610 x 2), margin -17.4 %**; ramp 165.64, helix 278.76 within | **the plunge entry** |

Every body, slot, cleanup, link and capped-step cut gives `NotApplicable(NoVerticalFeed)`.

### 13.3 Other predictions (reasoned)

- No feed moves: the op golden stays at the known 46 rows; FM1 moves no cell (its columns do not
  read `cut_checks`).
- `every_limit_judges_every_declared_cut_p29_7` claim 5 ("a plunge holds a number only for the
  spindle power") becomes "for the spindle power and the vertical rate": the claim text and its
  assertion change in e3.
- The MCP `get_safe_envelope` reply gains one `cut_checks` row per declared cut with the limit
  `plunge_rate` (an added row; no key changes). The viz pills read the same record.
