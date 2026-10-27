# Integration step 7.4a: the Geodesic Adaptive engine (option B, part F)

Date: 2026-10-28. Branch `geodesic-integration`, base `a82b79d70` (7.3d).
The implementer did not commit.

Patch: `step74a_tree_vs_a82b79d70.patch`, md5 `219ec6c662f67fc5133c255edc077768` (30 files; 12 tracked files: 1056
insertions, 203 deletions). It replaces the first freeze (md5
`012b46687838fd1f4bc930173cd15fd5`, the swept width only). `git apply --check -R` passes. The patch holds
every change of this step except this report.

Scope (lead brief and option B): `slice.rs`, parts E and G, part F inside
7.4, the 3D enum value, the path dispatch. The arm is reachable only from
tests: no config, GUI, CLI or MCP form names it (that is 7.4b).

## 1. What changed

| File | Change |
|---|---|
| `adaptive/geodesic/slice.rs` (new) | the 3D slice door: `slice_segments`, `slice_cleanup`, `slice_replay`, `slice_load_margin`, `link_length`, `HeldLinks`, and part F: `feed_caps`, `fed_runs`, `step_reading` (the larger of the swept width and the area per mm), `feed_factor` |
| `adaptive/geodesic/mod.rs` | the `pub(crate)` re-exports of the slice door |
| `adaptive/geodesic/tests.rs` | `the_area_term_binds_on_a_tight_turn` (§6.2) |
| `adaptive/path.rs` | `replay_after_group_filter` (`pub(crate)`, no gate) |
| `adaptive3d/outer_band.rs` (new) | G-CLIPAIR: `clip_to_stock_box`, `boundary_reach`, `RegionClip`, `clip_to_centre_reach` (R `research_outer_band.rs`) |
| `adaptive3d/centre_clip.rs` | `polygons()` for the outer band |
| `adaptive3d/clearing.rs` | three-way merge of R's slice layer; instruments deleted (§2); arm behaviour un-gated on `is_geodesic` (§3); `plan_one_outline` returns the caps |
| `adaptive3d/path.rs` | dispatch `AgentSearch \| ContourSpiral \| GeodesicAdaptive`; the emitter writes the capped feed and `Move::feed_cap`; ctx fields `region_clip`, `left_pieces` |
| `adaptive3d/mod.rs` | `ClearingStrategy3d::GeodesicAdaptive`; `effective_step_split` (arm: `StockDepth`), `effective_keep_down_route` (arm: `CutFeed`); `emission_chord_tolerance` (arm: capped at `slice_load_margin`); events `FeedCapStart { factor }` and `FeedCapEnd`; `mod outer_band` |
| `adaptive3d/keep_down.rs` | the `dead_code` expect on `keep_down_excess` is gone (the held links read it) |
| `compute/spans.rs`, `compute/annotate.rs` | the two new markers (no span, no annotation) |
| `adaptive3d/CLAUDE.md`, `adaptive/geodesic/CLAUDE.md` | file maps, invariants, sentries; 40 lines each |
| `tests/geodesic_adaptive_keeps_the_research_geometry_s7_4.rs` (new) | condition 1: the geometry pins (§4) and the instrument `write_the_per_move_feeds` |
| `tests/a_geodesic_cap_survives_every_dressup_s7_4.rs` (new) | condition 4: the cap census (§5) |
| `integration/step74a_feed_diff.py`, `.txt`, `step74a_feeds/` | condition 3: the feed diff (§6) |

`From<ClearingStrategy>` is unchanged: no config value maps to the arm yet.

## 2. Deleted (R = 607e52730; names, not spans: the files were merged)

- `adaptive3d/mod.rs`: the four env overrides (`RESEARCH_ARM_ENV`,
  `RESEARCH_ROUTE_ENV`, `RESEARCH_SLAB_FIELD_ENV`, `RESEARCH_STEP_SPLIT_ENV`
  and their `research_*_override` readers), `adaptive_3d_research_output_with_cancel`,
  `Research3d`, `output_and_turns`, `research_turn_feeds`, `xy_key`,
  `ResearchTurns`, `entry_records`, `EntryRecord`, `EntryKind`,
  `EntryCause`, `EntryCauseRecord`, `CoreZoneRecord`, `JoinsRecord`,
  `HeldLinkFault`, `RegionOpening`, `RouteStats`, `SplitStats`,
  `effective_slab_field`.
- `adaptive3d/clearing.rs`: `StampLog`, `StampLogTail`, `StampRecord`,
  `StampProbeLine`, `record_entry_cause`, `record_entry_causes_in`,
  `core_handoff_line`, the C29 pick (`research_pick_variants`,
  `RegionScratch`, `pick_region_plan`), `research_slab_field`, the
  duplicate `corridor_ceiling_z` / `keep_down_excess` (master has them in
  `keep_down.rs`, 7.2), and `check_held_link` with its probes.
- Behaviour not ported (by ruling): `LowRapid` (Q7), `SlabField`, the D50
  edge-op top (`rest_top_z`) and the D44 region publication. D44, D48 and
  D50 belong to the rest-region part of the design.
- Condition 5: `apply_slice_turn_feeds`, `apply_chain_feeds` and the
  TurnRecord matching do not exist in `src/` outside the research file;
  the research file keeps the 2D readout's own `TurnRecord` (test support,
  `research` feature).

## 3. Un-gated behaviour (was `is_research_arm`, now `is_geodesic`)

G-CLIPAIR boundary reach and region restriction; the waterline skip of
the pieces the arm leaves (`left_pieces`); `grown_outlines` (C31);
`leave_piece`; the held links through `keep_down::keep_down_excess`;
`link_length` in `filter_short_groups`; the 2D params with
`KeepDownLinks::WithinPassLoad`; the disc read. Every other strategy takes
the shipped branch: byte parity holds (§7).

## 4. Condition 1: the geometry equals R move for move

Sentry `geodesic_adaptive_keeps_the_research_geometry_s7_4`. Fixtures: the
island pocket of the research C12 judge (one level and two), its shelf,
and the channel. The step split and the route are set explicitly. The pin
is the move count and the FNV-1a hash of (target, move kind with feed 0,
intent). R ran the same test file in a scratch copy of 607e52730 with
`ClearingStrategy3d::Geodesic(C35_SPIRAL_PICK.pick_variants().0)`.

| Fixture | Moves | Hash | R = M |
|---|---|---|---|
| island pocket, one level | 978 | 14452218181729557987 | yes |
| island pocket, two levels | 2063 | 3602713506842892098 | yes |
| shelf, one level | 1056 | 14786195423799852005 | yes |
| channel, two levels | 0 | 675868731199239589 | yes |

The channel plans no move on both trees (R's C12 judge reads the same).

## 5. Condition 4: the cap census

Sentry `a_geodesic_cap_survives_every_dressup_s7_4`. It plans the island
pocket (planner segment merge on) and runs `apply_dressups` with every
step a 3D Rough allows switched on (link moves, arc fit, segment merge,
rapid order, air-cut filter with the board as prior stock, feed
optimisation), then the feed modulator (ConstrainedMax, full slot). It
asserts:
1. a fed line on the two ends of a planned cut carries that cut's cap;
2. a fed arc carries a cap that a planned cut at each end carries;
3. no fed move leaves the pipeline, or the modulator, over its cap.

Defects injected (then restored; `git diff` of `dressup/` is empty):
- arc fit writes `feed_cap: None`: red, "fed arc 18 ... the cap None is
  not a planned cap at both ends";
- feed optimisation ignores the cap: red, "the pipeline: fed move 1 has
  the feed 562.5, over its cap 404.38".

The per-stage proof stays `a_feed_cap_binds_every_feed_stage_c38`.

## 6. Condition 3: the feed diff, R against B

Lead ruling (2026-10-28): the reading of a fed step is the larger of the
swept width `a_e / D` and the area per mm over `D`, and the factor is
`c / max(swept w/D, area_per_mm / D)`. The first freeze read the swept
width only (S below). Condition 2 is widened to this reading.

Listing: `step74a_feed_diff.txt` (every differing fed move). Script:
`step74a_feed_diff.py`; it reads `step74a_feeds/{R,B,S}` and asserts the
counts below, that every slower run starts within `R` of a plunge, and
that every faster move has no cap in B. B = this tree (both terms).
S = this tree at the first freeze (swept width only).

| Fixture | Fed moves | Equal | B slower (departure runs) | B faster |
|---|---|---|---|---|
| island pocket, one level | 969 | 897 | 70 (18 runs) | 2 |
| island pocket, two levels | 1989 | 1848 | 135 (35 runs) | 6 |
| shelf, one level | 1050 | 977 | 70 (20 runs) | 3 |
| channel, two levels | 0 | 0 | 0 | 0 |

B/R on the slower moves: 0.346 to 0.829, 0.344 to 0.988, 0.346 to 0.868.
Fed time at the commanded feed (no acceleration), B over R: +3.7 %,
+3.6 %, +3.5 % (S was +2.0 %, +1.8 %, +1.9 %).

### 6.1 The slower moves: departure runs (conservative)

Each slower run starts 0.0 to 2.98 mm from the last plunge, so inside a
departure window (`R` = 3 mm). B caps the whole run piece at the least
factor of the windows on it. R fed each window by its own chain match.
So B is never over R's reading there. A cap per window needs a split per
window; it is not in this step. The runs are listed one by one in the
listing.

### 6.2 The faster moves, one by one

Each one has no cap in B: B's replay finds it on no planned turn and in
no departure window. R lowered it through its chain match by distance
(`apply_chain_feeds`, the counter `matched_out_of_run`). B is right on
each.

| Fixture | Move | From → to (mm) | R feed | B feed | Reason |
|---|---|---|---|---|---|
| island, one level | 873 | (20.043, -20.383) → (20.543, -20.383), z 4.5 | 496.47 | 1000 | a 0.5 mm step of a straight full-feed pass along the pocket wall; the departure at move 616 is 1.26 mm away; R's chain took this step |
| island, one level | 879 | (24.043, -20.383) → (24.543, -20.383), z 4.5 | 496.75 | 1000 | as 873; departure 648, 1.28 mm away |
| island, two levels | 927 | (20.043, -20.383) → (20.543, -20.383), z 7.0 | 496.47 | 1000 | as 873, upper level; departure 654, 1.26 mm |
| island, two levels | 930 | (22.543, -20.383) → (23.043, -20.383), z 7.0 | 380.52 | 1000 | as 873; departure 654, 1.24 mm |
| island, two levels | 933 | (24.043, -20.383) → (24.543, -20.383), z 7.0 | 496.75 | 1000 | as 873; departure 688, 1.28 mm |
| island, two levels | 936 | (26.543, -20.383) → (27.043, -20.383), z 7.0 | 359.63 | 1000 | as 873; departure 688, 1.22 mm |
| island, two levels | 1934 | (22.049, -20.377) → (22.549, -20.377), z 4.5 | 511.06 | 1000 | the same wall pass, lower level. R fed the departure at move 1709 (1.34 mm away) at 1000 and lowered this step in its place; B caps 1709 at 380.5 |
| island, two levels | 1940 | (26.049, -20.377) → (26.549, -20.377), z 4.5 | 493.53 | 1000 | as 1934; R fed departure 1743 at 1000, B caps it at 359.6 |
| shelf, one level | 390 | (8.543, 2.124) → (9.043, 2.241), z 10.5 | 752.91 | 1000 | a travel move at clearance height (z 10.5, over the 10.0 stock top) before the plunge 391; it starts at the XY of the capped cut 388 (z 4.72), and R's chain took it as a step of that run. It cuts no stock |
| shelf, one level | 927 | (20.043, -20.383) → (20.543, -20.383), z 4.5 | 496.47 | 1000 | as 873; departure 649, 1.26 mm |
| shelf, one level | 933 | (24.043, -20.383) → (24.543, -20.383), z 4.5 | 496.75 | 1000 | as 873; departure 691, 1.28 mm |

### 6.3 Why both terms (the code doc of `slice::step_reading`)

- The swept width is the engagement across the path: the planner's width
  read, the rule the replay admits a step by.
- The area per mm (`PassLoad::area_per_mm`) is the removal per mm of
  travel. That is the quantity of the one load `L`
  (`tool_load::chip_load_fraction`: chip area per strip over the
  full-depth slot). On a curve the path turns inside the material, and
  the swept width under-reads it.
- On a straight full-depth cut the two agree (on the lattice, to one cell
  over `D`); test `a_full_pass_reads_the_extent_over_the_diameter`.
- Unit test `the_area_term_binds_on_a_tight_turn`: a trail along +x beside
  a one-stepover side cut, then a 30° turn in one pass step. It reads
  width 0.653 and area 0.667 over the ceiling 0.363. The factor is
  `c / (area_per_mm / D)` bit for bit. With the area term removed, the
  test is red.

## 7. Gates (all through the worktree `scripts/cargo_lane.sh`)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features (heavy-tests, research, test-support) | ok |
| `adaptive3d_emission_byte_parity` | 5/5 |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1 |
| the 14 adaptive3d sentries of `adaptive3d/CLAUDE.md` | all ok |
| the two 7.4 sentries, `a_feed_cap_binds_every_feed_stage_c38` | ok |
| `--lib` filter `adaptive` (plain; and `research`), with `the_area_term_binds_on_a_tight_turn` | 155 ok, 7 ignored; 162 ok, 8 ignored |
| `the_geodesic_preset_keeps_its_plan_s7_3` (`research`) | 2/2 |
| `folder_maps_name_real_files_f140` | 4/4 |
| the 2D adaptive sentries of `adaptive/CLAUDE.md` | ok, except the two G-MASTERREDS-1027 reds: `g_adaptpassload` (:703, same moves 2810/6159) and `g_adaptrapidlift` (:188), as on master. Run before the area term; not run again, because `slice.rs` is reached only by the arm |

## 8. Pre-registered for D10 (after 7.4b; R numbers: D45, D47, D52, D54)

The geometry is R's plan A, and the pick kept plan A in every region of
both parts (D52). So every geometry-only reading of op 1 must equal R's
number exactly (D52: run-to-run spread 0). A gap is a port defect.

| Op 1 reading (geometry only) | Corne B s2.1 | wanaka s2.0 | wanaka s3.5 |
|---|---|---|---|
| L max / L p99 (area load, part A) | 0.999 / 0.493 | 0.720 / 0.375 | 0.992 / 0.642 |
| mm over own bound | 171.6 | 107.4 | 16.2 |
| G-code plunges | 612 | 1262 (D52 pick on) | — |
| descent events | 382 | 581 | — |
| removed volume (sim, mm³) | — | 591516 | 577240 |

The feed changes by design (option B), so these may differ from R:
program md5 and the op-1 acceleration clock (R: Corne 1075.5 s, wanaka
s2.0 2261.9 s, s3.5 1323.6 s). Expected: op 1 slower by about 3.5 % (the
fixture fed-time ratio, §6), never faster than R except on R's chain-match
moves (§6.2). A clock over R by more than 6 % is a finding to explain
before 7.4 ships.

Not measurable after 7.4b: the edge op on `DerivedRestRegions` (D44 and
D50 are not ported), so the job clocks of D47/D54 (wanaka 0.32 / 0.57 of
CP, Corne 1.11) wait for the rest-region step.
