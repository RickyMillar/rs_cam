# Review of 5ed089621..109ed6644 (crates), by diff

Reviewer: independent agent. HEAD 109ed6644. No product code changed.
The working tree holds one change only: the b5 test fixture (`b5.patch`).
Operator file sha256 before and after: `f6a2b388...d72d599e4e41` (unchanged).

## Part 1: first-hand move comparison (wanaka operator file)

Probe: `zz_review_probe.rs.txt` (deleted from the tree after the run).
It loads `nz_wanaka_21_operator.toml` through `ProjectSession::load`,
generates idx 0, 3-13, 17 with no simulation, dumps the moves
(`now_<idx>.csv`, same columns as the old dumps) and exports through
`export_gcode_checked(None)` (`export.nc`). Built `--release`.

Old dumps `p29_8e/head_*.csv` date from 2026-11-13 07:10-07:14. That is
after 0038733a5 (06:48) and before 35f8e1a56 (8b, 09:09) and 0fad0599d
(8c). Their entry feeds (EntryApproach 1081 = half of 2162) confirm "before 8c".
No `head_3.csv` exists, so idx 3 has no baseline.

| idx | op | moves before | moves after | differences | verdict |
|---|---|---|---|---|---|
| 0 | Face back to T1 | 317 | 317 | 60 EntryApproach feeds 1081 -> 2162; no position change | expected (8c) |
| 3 | Pin drill, flip pins | - | 118 | no baseline | UNMEASURED (no old dump) |
| 4 | Drill light holes | 463 | 463 | 0 | equal |
| 5 | River channel 6 mm | 17261 | 17261 | 0 | equal |
| 6 | Lake pocket 11 lakes | 30111 | 30111 | 0 | equal |
| 7 | Lake pocket 1 lake | 194 | 194 | 0 | equal |
| 8 | ANTENNA pocket | 640 | 640 | 9 EntryApproach feeds 120 -> 240; no position change | expected (8c) |
| 9 | V rivers and lakes | 22369 | 22369 | 0 | equal |
| 10 | Day strip slit V60 | 19 | 19 | 0 | equal |
| 11 | Face clamp pads | 64 | 64 | 8 EntryApproach feeds 1081 -> 2162 | expected (8c) |
| 12 | Face front to T | 371 | 371 | 2 EntryPlunge feeds 1081 -> 2162 (moves 219, 266) | expected (8c) |
| 13 | Engrave day labels | 314 | 314 | 0 | equal |
| 17 | Outline profile with tabs | 1750 (head) / 1840 (e1_17) | 1840 | vs head: e1 + 8c; vs `e1_17.csv`: 0 differences | expected (e1), equal to e1 |

idx 1, 2, 14, 15, 16 stay UNMEASURED (each takes minutes; excluded by the brief).
8b moved no op of this file: every op states `ramp_angle = 3.0`.

Verdict: no unexpected difference in a position, a feed or a move count.

G-code check (`export.nc`, 74259 lines, `gck.py`, `fcmp.py`):
- 0 non-ASCII bytes. 0 comment lines with an inner or unbalanced parenthesis.
- F words: 2678, min 240, max 6488; all finite and positive; all under the
  machine ceiling 15000. No G0 line has an F word.
- The modal F of every G1/G2/G3 line equals `round(dump feed)` for every
  move of all 13 ops (0 mismatches over 70 011 fed moves). The dumps hold no
  G2 or G3 move, so no arc centre was compared.
- The new P30-1 header lines (`(feeds: ...)`, `(feeds run ...)`) are at most
  86 characters (measured).
- Line length FAILS the brief's check: the new warning line
  `export.stated_feed_over_limit` (30a037567) is 128 characters, the longest
  line of the file; the longest line of a kind that existed before is 124
  (an `op_left_out` line). Both lengths grow with the op name. No line
  limit of the controller is stated in the repo that I checked; see F12.
- The header ranges equal the dump ranges (idx 5: 2512 to 3165; idx 6: 990 to 1010).

## Findings, ranked by what reaches the machine

### F1. DEFECT (reproduced at `apply_entry`): a fold ramp cuts into a tab below the tab top
- Site: `dressup/mod.rs` `apply_entry_at` Ramp arm -> `entry_descent.rs`
  `collect_following_cut` (line 151), `walk_fold_path` (line 319),
  `fold_ramp_points` (line 362).
- Mechanism (read in the code): `collect_following_cut` takes the tab lift (a
  vertical Linear UP) and the tab traverse into the follow run; it stops only
  at a rapid or a downward plunge. `walk_fold_path` skips a segment of zero
  XY length (`seg <= 1e-12 => continue`), so the tab-top vertex of the lift
  is dropped and the low vertex at the same XY stays. The fold clamps
  `z >= b.z` only at a vertex, so the return chord to the low vertex
  crosses the tab span below the tab top.
- Reproduction (temporary lib test, deleted; copy in
  `dressup_tests_with_tab_probe.rs.txt`, test `zz_review_fold_over_a_tab`):
  plunge at (0,0) to z -2, cut to x 3, tab lift to z -0.5, traverse to x 20,
  down to -2, cut to x 40; `apply_entry`, Ramp 5 deg, stock top 0, r 3.
  Output ramp: (3, 0.24) -> (14.29, -0.5) -> (3, -1.74) -> (0, -2).
  19 samples over the tab span (x 3.56 to 13.72) are below z -0.5; the
  deepest sample is z -1.676 (1.18 mm into the tab). The test calls
  `apply_entry` direct, not a Profile op.
- Age: the fold mechanism is older than the range (G-RAMPCONTAIN, R10).
  The range adds new sites: 910ded4b4 sends every clip re-entry of a
  Ramp or Helix op into it. d60ab8aa4 lengthens the walk only where the
  budget is under the tool radius (20 deg).
- Reach: an entry or clip re-entry less than the walk before a tab start.
  The walk out is `half_len = ramp depth / tan(angle) / 2` (`emit_ramp`,
  the full depth from the clearance over the material), at least the tool
  radius: 14.29 mm in the reproduction (2.5 mm drop at 5 deg).
- Operator file: idx 17 at HEAD has 0 ramp samples inside a tab span below
  the tab top (`tabchk2.py`), so this file is not hit today. No test covers
  a fold over a tab.

### F2. DEFECT (reproduced by a sentry that passes): every ramp row gets `Caveat::WorstCase`, which softens Exceeds
- Site: `limits/row.rs:744-746` pushes `WorstCase` for any row with
  `own_depth.worst_case`, with no test of the limit; `row.rs:353-360`
  `softens()` lists `WorstCase`.
- The ramp chip and the ramp vertical feed do not depend on depth, so these
  values are exact. A ramp `ChipMax` or `PlungeRate` row over its bound reads
  "advisory". `PlungeRate` has no sample gate (`S-NoSampleGate`), so no later
  check shows it.
- Reproduction: claim 5 of `tests/one_record_on_every_surface_p29_10.rs`
  (lines 966-990; ran: 10 passed). A ramp `ChipMin` row at half the band
  feed is `Exceeds`, carries `WorstCase`, and the test asserts its tone is
  `Tone::of(check, [WorstCase, ..])`, the softened tone. The chip there is
  a depth-free value.
- Sentry gap: that assert computes the expected tone with the product
  function, so it cannot catch this.

### F3. RISK (agent trace, corrected by me): a planned deflection row with no width or chip can read Within
- Site: `feeds/planned.rs:845-858` `DeflectionPlanned::judge` returns `None`
  both for "no root below the flute" and for `width_and_chip == None`.
  `feeds/envelope.rs` arm `(Judged(root), Body | Own(_)) => Ok(root)`, then
  `Ok(None) => CheckState::Within` (line ~1805).
- The agent's input (an Adaptive3d body with no depth) does NOT reach it:
  the guard above (`Body if value.is_none()`, `value = value_of(e.depth)`)
  gives `NotModelled`. Left open: a body cut with a depth value but no chip
  or width, and an `Own` entry cut whose chip is `Err`. Not run.

### F4. RISK (agent trace, read by me): a lower-bound depth over its ceiling softens to advisory
- Site: `envelope.rs:1780-1795` keeps `Exceeds` when `lower_bound`;
  `row.rs:747-749` adds `LowerBound`; `softens()` lists it. `envelope.rs`
  `stated_value_check` (1068-1082) states the rule "a lower bound proves
  only Exceeds", so an Exceeds at the lower bound is certain, yet its tone
  is `ExceedsAdvisory`. Input: a Project Curve whose lower-bound depth is
  over the deflection root. Sentry claim 7 of
  `every_limit_judges_every_declared_cut_p29_7.rs` checks the state only.
  Not run.

### F5. RISK: the export header warns only for the chip-max row
- Site: `session/limit_rows.rs:188-229` keeps only stated rows that are
  `Exceeds` AND carry `UntilModulated`; `UntilModulated` exists only for
  `capped_limits` (`ChipMax`). `export.rs` `Gate::UnmodulatedOverLimit`.
- A stated power, deflection, force-budget or vertical-rate row over its
  bound, with no simulation, puts no "over a limit" line in the G-code
  (I read every gate arm of `export_warnings`, `export.rs:187-410`: the
  load rows need a simulation; `Feasibility` holds physical rules only).
  The header then says only "<metric> not measured". On this file the two
  warnings are both chip max. Input that shows it: any op whose stated
  deflection row is Exceeds and that has no simulation.

### F6. RISK (agent): a cut depth past the search span reads Within
- Force budget planned side: "no root below the flute" -> `Ok(None)` ->
  Within (`envelope.rs:3668-3672`, `1805`); the gate side has the deeper-than-flute
  rule (`tool_load/depth.rs:519-557`). Deflection search stops at
  `min(0.95 x stickout, flute)` (`cutter_constraints.rs:331-336`).
  Input: a stated depth above the flute length. Not run.

### F7. RISK (agent): a MayOccur cut that does occur stays advisory with no later gate
- `row.rs:360` (`MayOccur` softens). `PlungeRate`, `RubbingFloor`,
  `VendorDepthTable` have no sample gate. Input: Helix entry in a pocket too
  small for the helix; the fallback always runs; its vertical rate row over
  the axial-chip arm reads advisory before and after the simulation.

### F8. RISK (agent): an unmodelled sample is dropped when another sample has a cap
- `tool_load/depth.rs:304-306, 326-327, 351`: `unmodelled` is read only when
  `judged` is `None`. One refused sample (deeper than the flute, no peak
  force) beside a capped sample with ratio < 1 gives Within with no word on
  the refused one.

### F9. RISK (agent): proposal rows soften under a current modulation they do not have
- `envelope.rs:3365` passes the current `RunModulation` to the proposal rows.
  Apply clears the result, so the export carries the proposal feed with no
  modulation; the preview said "before modulation", advisory.

### F10. NOTE: the b5 guard was dead; repaired; its claim holds on its fixture
- `an_upstream_change_supersedes_the_downstream_modulation_b5` refused in
  its own setup since 74af0f8e1 (S11, 2026-10-23: `feed_rate` and
  `plunge_rate` became `Option`, the fixture's `toolpath_config` got
  `fill_unset`, but the test built `edited.operation = pocket_to(2.0)` raw).
  `git log -S "fill_unset(&mut op)"` names 74af0f8e1; the test file was not
  touched after 7f820c42e (2026-10-15).
- Repair (`b5.patch`): the edited op goes through
  `compute::fixture_values::fill_unset`, the function that fills every other
  row of the chain. No new number.
- Result: 2 passed, 0 failed. The precondition (Finish `ModulatedCurrent`)
  and the claim (`ModulatedSuperseded { run, current }` and its statement)
  both hold.
- The named symptom ("modulated, run 1, not current" survives a new
  simulation): a temporary third test (copy
  `b5_with_resim_probe.rs.txt`, deleted) ran a FULL simulation after the
  superseded state. Result: after the partial run
  `ModulatedSuperseded { simulation_run: 3, current_run: Some(4) }`; after
  the full run (run 5) `ModulatedCurrent { simulation_run: 5 }`. So the
  symptom does not reproduce on this fixture. On rivmap100 Face 5 it stays
  UNPROVEN (no project simulation was allowed). `b5.patch` equals the tree
  diff after the probe was removed.

### F12. NOTE (fails the brief's G-code check): the new export warning line is the longest line
- `export.stated_feed_over_limit` (30a037567): 128 characters in
  `export.nc` line 65, over the older maximum of 124. Its length grows with
  the op name and the row face. No test bounds a header line length.

### F11. NOTE: drill ops and air leads are in no class of `EmittedFeeds`
- `session/emitted_feeds.rs` `figures_of`: a Drilling move and an
  EntryApproach move are counted nowhere. The drill phases of `export.nc`
  (lines 394, 520) print only "(feeds: as planned, not modulated)". The
  claim "every surface prints the feeds that run" does not cover them. The
  F words are correct; only the record is partial.

## Part 2: per commit, what I checked

- e88fe9491 (one entry rule): `ResolvedEntry::of_dressup` / `of_adaptive3d`
  are the only builders; both emitters read it. PROVEN by sentry
  `one_entry_rule_two_emitters_p29_8` (not run by me).
- 35f8e1a56 (5 and 20 deg): the cited text matches
  `planning/feeds_contract_2026-11-03/research/sources_entry_rules/easel_toolpath_ramping.txt`
  line 13 word for word. A 3D Rough file with no `ramp_angle_deg` moves from
  10 deg to the rule (wire break stated in the commit). No operator op uses
  the rule. Source VERIFIED by reading; no move of this file depends on it.
- 0fad0599d (op plunge rate): every vertical fed descent of the 13 dumps runs
  at its op `plunge_rate` (Drilling at the drill feed). The removed
  first-linear-feed fraction is gone from every caller. PROVEN on the
  operator file (measured above). The plunge
  feed is not capped at the Z axis rate ($112) in the program; NOTE
  only (on this file every plunge is under 2500).
- e6b587a2e: `feed_before_write` reads the unrounded calculator feed only
  when the stored feed equals the written feed. G-RAMPCLAMP still holds the
  ramp at or below the cut feed. By reading; UNPROVEN by a test I ran.
- 01a2ff1cc: agent: the `StoredFeed` caveat sits only on a Within row; at
  most 2 mm/min is hidden, on a floor check only; the two-write derivation
  is now out of date (Suggest rounds once) and a manual feed also gets the
  interval (`entry_rule.rs:879` tests `is_some()`). No false Within found.
- 910ded4b4 (clip re-entry fold), case by case:
  - Index through boundary clip, obstacle clip and the K3 start rapid:
    PROVEN by `compute/obstacle.rs` test
    `a_boundary_reentry_is_carried_through_the_obstacle_clip_p29_8e`
    (asserts `[r + 1]` after K3, and the ramp at that site). By reading:
    `clip_walk` records before the push, a carried descent is recorded once
    (`else if` chain), the floor-only pass keeps the list.
  - Helix style -> fold ramp, no helix, inside the boundary: PROVEN by
    `a_helix_op_reenters_a_boundary_by_the_fold_ramp_q9`.
  - Leaves the boundary: the fold reads only the kept cut and stops at the
    exit rapid. UNPROVEN by test beyond the Q9 test (one Pocket box).
  - Obstacle keep-out: the K1 backstop clip runs after the entry step
    (`session/compute.rs:1242-1257`) and splits any fold that meets an
    obstacle; the obstacle permit is 3D (`ObstacleToolView::allowed_intervals`,
    `obstacle.rs:1092`). UNPROVEN by a test of a fold that meets an obstacle.
  - Surface-riding fresh op -> plunge; rest surface op -> `RestEntryRamp`;
    Planner contract (3D Rough) -> effective style None -> plunge: by
    reading only (`clip_entry.rs` `reentry_owner`,
    `dressup_apply.rs:782-792`). UNPROVEN by test.
  - Flipped setup: not reviewed; UNPROVEN.
  - Re-entry on an arc: the arc is dropped and the plunge lands at its end
    (behaviour before the range). UNPROVEN by test.
  - Zero-length kept piece: fold run 0 -> plunge at the plunge feed; no panic
    by reading. UNPROVEN by test.
  - Tab: DEFECT F1.
  - Feeds: no usable plunge feed gives `f64::INFINITY`, and every use is
    `feed.min(INFINITY)`, so no NaN or infinity reaches a move (reading).
    On the operator file every vertical fed descent runs at its op plunge
    rate (measured, 13 ops).
- 187c4ee8d: the tab descent reads `EntryPlungeFeed`: PROVEN by
  `--lib a_tab_descent_reads_the_plunge_feed_of_the_entry_rule_p29_8e`
  (not run by me). Under Ramp the pipeline turns it into a forward fold
  (seen in my F1 probe). The fold BEFORE a tab is F1.
- d60ab8aa4: `fold_walk_length = max(budget, min run)`; it lengthens the walk
  (feeds F1).
- 30a037567: header lines are ASCII and parenthesis-free on this file;
  `EmittedFeeds` reads the post's `emitted_feed` (clamp then round) and its
  memo key holds the stated feeds, decimals and max feed. Sound apart from
  F5 and F11.

## Part 5: structure (at most ten, ranked)

1. The op-type guard is a token scan (`tests/common/op_type_scan.rs`, 17
   tokens, `use` lines cut out). It does not see: `use ...::OperationType as
   Kind;` then `Kind::Pocket`; a glob `use OperationConfig::*;` then
   `Pocket(c) =>`; a helper method on a type outside the guarded files
   (`compute/catalog`) that reads the op type and that a limit calls by a
   neutral name; `Self::Pocket` inside an impl of `OperationConfig` in a
   guarded file.
2. F1 shows the fold has no type that holds "the ramp stays on the kept cut
   above it". The rule lives in `collect_following_cut` and the vertex
   clamp only.
3. `Reentering` claims "a new clip cannot skip the entry rule", but
   `Reentering::none`, `stay_plunges` and `PlungesStay::NoOperation` are all
   `pub`; any door can leave re-entries as plunges by naming `NoOperation`.
4. `EntryPlungeFeed::of_op_plunge_rate(Option<f64>)` is `pub` and takes any
   number. The type holds "finite and positive", not "the op plunge rate";
   the doc says "a new descent source takes this rule or does not compile".
5. Second plunge door: `RestEntryRamp { plunge_rate: self.context.plunge_rate }`
   (`session/compute.rs:1220`) reads the raw context value, not
   `ResolvedEntry::plunge_feed`.
6. The export over-limit line is a function of one limit (F5), not of every
   stated row.
7. `EmittedFeeds` classes leave out drill and approach moves (F11).
8. Three sentries assert on values that the product function computes on
   both sides (agent: `one_record_on_every_surface_p29_10.rs:986`,
   `every_declared_cut_is_judged_at_the_stated_point_p29_10.rs:364-379`,
   `a_row_softens_only_under_a_current_modulation_p30_2.rs` test 2).

## Not reviewed

- idx 1, 2, 14, 15, 16 of the operator file (minutes each); idx 3 has no baseline.
- No simulation of any project; the rivmap100 Face 5 report stays unproven.
- The viz and CLI diffs of 30a037567; the P29-10/11 commits outside the
  list beyond the rows the agent read.
- F2-F9 come from a second agent's code trace, spot-read by me
  (row.rs:735-752, 345-365; envelope.rs:1798-1820, 3755-3780;
  planned.rs:840-860); no test was run for them.

## Files in this folder

`now_<idx>.csv` (dumps), `export.nc`, `probe_log.txt`, `cmp.py`, `gck.py`,
`fcmp.py`, `tabchk.py`, `tabchk2.py`, `zz_review_probe.rs.txt`,
`dressup_tests_with_tab_probe.rs.txt`, `dressup_tests.rs.orig`, `b5.patch`.

## Lead rulings (2026-11-15)

The reviewer did not write the code under review. The lead takes every finding as stated.

- **F1 (a fold ramp cuts into a tab): fix now.** It reaches the machine. A fold ramp stays at or
  above the cut it follows along its WHOLE path, not only at the vertices, and the cut it follows
  ends at a rise (a tab lift). Predictions first; the operator file must not move (idx 17 has no
  hit); a moved operator op stops the work.
- **F2 (`WorstCase` on every ramp row): fix now.** The caveat holds only on a row whose value
  depends on the ramp's own depth. A chip row and a vertical-rate row of a ramp are exact and keep
  their tone. The sentry claim asserts a literal tone, not the product function.
- **F3 (`LowerBound` softens a certain Exceeds): fix now.** A row that exceeds at a lower bound
  exceeds for certain: no softening. The caveat softens nothing; it explains a `Within` that is
  not proven.
- **F4 (the export warning names only the chip maximum): fix now.** With no current measured
  verdict for an op, the export warns for every stated row with a gating Exceeds tone, whatever
  the limit. A warning, never a refusal.
- **F5 and F6: fix now where the fix is a row state or tone, with predictions.** "No chip or
  width" is `NoNumber` with a reason, never `Within`. Past the flute or the search span a row is
  not `Within` by default. A proposal row does not soften under a modulation that it will not
  carry after an apply. A sample with no model in the depth gate is counted and named. The
  "may occur" question (a cut that does occur stays advisory and no gate judges it) is logged for
  the operator with the list of affected rows.
- **F8 (`EmittedFeeds` leaves out drill moves and air leads): fix now.** Every fed move is in a
  class; a move in no class fails a test.
- **F9 (structure): fix the guard's four blind spots and narrow the `pub` items now** (a
  byte-equal commit). The second plunge door of `RestEntryRamp` and the sentries that assert on
  the product function go to the clean-up list with the rest of `P27_BRIEF.md`.
- **Part 1.** The lead's first-hand statement for the operator: ops idx 0, 4 to 13 and 17 of the
  operator file are move-for-move equal to the dumps of 2026-11-13 except the ruled entry feeds
  (8c) and the ruled re-entry ramps of idx 17. Idx 1, 2, 3, 14, 15 and 16 are not measured.
