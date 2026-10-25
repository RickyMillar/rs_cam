# P3 C22 — the 3D arm runs the two KEEP columns (turns, turn feeds, held links)

Agent C22, 2026-10-24. Worktree `.claude/worktrees/adaptive-head`, branch
`adaptive-instruments`, base HEAD cdc9814e. Not committed. No cargo command
ran. `rustfmt --edition 2024 --check` is green on the five changed Rust
files. Every outcome below that is not a line of code is a prediction and
is marked **P**.

Paths: `R` = `crates/rs_cam_core/src/adaptive/research_geodesic.rs`,
`C` = `crates/rs_cam_core/src/adaptive3d/clearing.rs`, `M` =
`crates/rs_cam_core/src/adaptive3d/mod.rs`, `PA` =
`crates/rs_cam_core/src/adaptive3d/path.rs`, `T` =
`crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs`. Line numbers are
of the C22 tree.

**Shared tree.** C23 edits the rest of `R`, `adaptive/path.rs`, `search.rs`
and the 2D harness at the same time. My change in `R` is only the C12 block
(`// ── The 3D research arm (C12) ──` to the end of `HeldLinks`, R:7249-7570
now). I call four items outside it that C23 can change: `cleaned` (R:7024),
`pass_load_readout` (R:6068), `PartIndex::of_turns`, `holding_move`, and
the types `TurnRecord`, `TurnSummary`, `TurnStep`. A signature change there
breaks my block at compile time, not in silence.

## 1. The route

The 2D run (`run_2d_with`, R:7083) does four things with the turns:

1. `geodesic_segments` returns `Produced::turns`.
2. `cleaned(.., turns)` replays the kept segments with `LinkLoad::turns`
   (`apply_contour_parallel_residue_cleanup_turns`), so the replay admits a
   planned turn over the ceiling (`admits_turn`, bound `turn_bound`).
3. `pass_load_readout` with `PartIndex::of_turns` reads each turn's steps
   (swept width, area per mm).
4. `apply_turn_feeds` writes `F · turn_feed_factor` on each `Linear` move
   that lies on a turn, and counts fed / unfed / full-feed-unmatched.

Before C22 the 3D arm dropped the turn list at step 1 (`slice_segments`
returned only `segments`), ran the shipped cleanup (empty turn list), and
had no feed step. C22 carries the same four steps through the 3D slice:

| Step | 2D | 3D (C22) |
|---|---|---|
| 1 | `Produced::turns` | `SlicePlan { segments, turns }` from `slice_segments` (R:7253, R:7268); `research_slice_segments` returns it (C:2209); `plan_one_outline` keeps `research_turns` (C:2328-2337) |
| 2 | `cleaned(.., turns)` | `research_geodesic::slice_cleanup` = `cleaned(.., SweptWidth, turns)` (R:7288), called only when the turn list is not empty (C:2357-2371). Else the shipped cleanup runs, moved unchanged into `fn slice_cleanup` (C:2484) |
| 3 | `pass_load_readout` + `PartIndex::of_turns` | `slice_turn_records` (R:7304): the same readout of the cleaned plan on the planner cell, SweptWidth, one `TurnRecord` per turn with steps, `turn_feed_factor` (C:2373-2381) |
| 4 | `apply_turn_feeds` on the piece toolpath | the records go to `ctx.research_turns` after the F-038 filter (`SliceTurns::add`, R:7377; C:2471-2478), out through `Adaptive3dSegmentsResult::research_turns` (PA:600, PA:976, PA:1169), and `apply_slice_turn_feeds` (R:7416) writes the feeds on the emitted 3D toolpath (M:636-647, `output_and_turns` M:657) |

So the 3D replay admits a turn exactly as the 2D replay does: it is the
same function (`cleaned`) with the same list, the same `LinkLoad` and the
same `turn_bound`. The residue contours, the wall pass and the mop hold the
ceiling with no exemption, as in 2D.

## 2. Changes

### `research_geodesic.rs`, C12 block only (R:7249-7570)

- `SlicePlan` (R:7253) and `slice_segments` (R:7268) now return the turns.
- `slice_cleanup` (R:7288): the 2D research cleanup.
- `slice_turn_records` (R:7304): the turn records of a cleaned slice plan.
  `feed_mm_min`, `fed` and `holding` are set at emission.
- `turn_in_plan` (R:7356): a `Cut` step with both ends within 1e-6 mm of
  the turn's segment (the walker's same-point tolerance).
- `SliceTurns` (R:7369) and `SliceTurns::add` (R:7377): `planned` adds the
  producer's turn count (as `apply_turn_feeds` does); the records stay only
  where the F-038-filtered plan still cuts them. Reason: the F-038 filter
  drops whole groups. A turn in a dropped group has no move, and the 2D
  rule records only turns that the plan cuts.
- `apply_slice_turn_feeds` (R:7416): the rule of `apply_turn_feeds` on the
  3D moves, in XY. A `Linear` move whose start and target both lie within
  1e-6 mm of a turn's segment in XY carries `min(f, F · factor)`. A turn
  with no such move is unfed when factor < 1, else
  `full_feed_unmatched` (C20), and `holding_move` names the move that holds
  it. The moves are bucketed by turn length (the longest turn is the
  bucket side, floor `CHAIN_EPS`), so a move tests only the turns at its
  start. Why XY only: the emitter drapes Z after the plan. A move of
  another level matches only when both its ends lie within 1e-6 mm of the
  turn's segment. Two levels plan the same turn only from the same outline,
  and the 2D plan and its readout read the outline alone, so both give the
  same factor.
- `link_length` (R:7488): the length of a group's `Link`s, for the F-038
  filter (see §3).
- `HeldLinks` (R:7529): replaces `links_as_cuts` (§3).

### `adaptive3d/mod.rs`

- `research_arm_override` (M:123-131): `geodesic_local_oneway` →
  `Geodesic(GeodesicOptions::C16_LOCAL_ONEWAY)`, `geodesic_local_zigzag` →
  `Geodesic(GeodesicOptions::C20_LOCAL_ZIGZAG)`. The two C12 values stay.
  Docs of the variant and of `RESEARCH_ARM_ENV` name the four values.
- `adaptive_3d_toolpath_output_with_cancel` (M:593) is now
  `output_and_turns(..).map(|(out, _turns)| out)`. `output_and_turns`
  (M:657) is the old body plus one line, `research_turn_feeds` (M:636 with
  `research`; M:649 without, which returns the toolpath unchanged). The
  type alias `ResearchTurns` is `TurnSummary` with `research` and `()`
  without. `Adaptive3dOutput` is unchanged, so `compute/execute/rough_3d.rs`
  (an exhaustive destructure) needs no change.
- New, research only: `pub fn adaptive_3d_research_output_with_cancel`
  (M:609) returns `(Adaptive3dOutput, TurnSummary)`. The tests call it.
  The CLI and GUI go through the shipped door and get the same feeds
  written on the toolpath; they do not see the summary.

### `adaptive3d/path.rs`

- `Adaptive3dSegmentsResult::research_turns` (PA:600), the context field
  initialiser (PA:976) and its hand-over (PA:1169). All three are
  `#[cfg(feature = "research")]`.

### `adaptive3d/clearing.rs`

- `ClearZLevelContext::research_turns: RefCell<SliceTurns>` (C:415, cfg
  research). The context is built once (PA:944) and used on one thread
  (no rayon in `adaptive3d/` after the surface heightmap).
- `plan_one_outline` (C:2328-2390, C:2433-2437, C:2471-2478), as in §1.
- `fn slice_cleanup` (C:2484): the shipped cleanup match, moved verbatim.
  The `Legacy` arm returns `to_vec()` of the slice (the slice always sets
  `ContourParallelHybrid`, C:3090 area, so the arm is not reached).
- `clear_one_region` (C:2757-2768, C:2822-2826): `HeldLinks` (§3).

### `tests/adaptive3d_geodesic_arm_c12.rs`

- `plan` now takes the params and calls
  `adaptive_3d_research_output_with_cancel` (T:285).
- New class in the oracle, the clearing turn (T:299 `turn_moves`, use in
  `read` after the vertical class): samples on a `Linear` move whose XY
  ends lie on a turn are counted (`turn`, `turn_screened`, `turn_peak`) and
  not judged against the bound. That is the 2D rule: the replay admits a
  turn over the ceiling and the readout counts it apart (`turn_over`).
- `turn_faults` (T:335): unfed > 0; a turn longer than `turn_bound()`
  (T:207, `s + (1 + √2)·cell` = 3.207 mm, the C14 derivation); a turn move
  whose feed is over `F · factor`; a fed turn with factor < 1 that no move
  carries.
- The heavy test (T:786-855) runs four modes: `geodesic_curv`,
  `geodesic_curv_split`, `geodesic_local_oneway`, `geodesic_local_zigzag`.
  Per mode: the two C12 assertions, `turn_faults` empty, and
  `planned > 0` exactly for the two arc-link modes. It stays `#[ignore =
  "heavy: .."]` (the reason now says about 12 min).
- New plain test `the_geodesic_arm_feeds_its_turns_down_at_the_level`
  (T:885): the same pocket, one level (Depth/Pass = `TOP_Z - FLOOR_Z`, so
  the only level is Z 4.5), both arc-link modes, no simulation. It asserts:
  turns planned; `turn_faults` empty; at least one turn with factor < 1 is
  fed; every turn move has both ends within `TOLERANCE` of Z 4.5. It prints
  the entries (`entry_count`, runs of entry intent).

No new constant. Every number is the planner cell, the stepover, the
tolerance, the 1e-6 mm same-point tolerance of the walker, `CHAIN_EPS`, or
a fixture value already in the test.

## 3. Keep-down links and turns in 3D: the finding

**Question (brief item 3).** Do the arm's arc-to-arc keep-down return
links and its turns stay at cut depth?

**Turns: yes.** A turn is a step inside a 2D `Cut` (`geodesic_segments`
joins `w.pts` with `run = [cur]`; the turn is the gap `cur → w0`, at most
the reach). In 3D the lift keeps XY exactly and sets Z to the level floor
(`lift`, C:2541). A turn cuts stock (that is why it needs the
exemption), so the disc read (C18, C:2802-2820) calls it engaged, and the
air demotion does not touch it. The only way a turn leaves the level is
the lift's large-Z split (`PLUNGE_SLOPE_LIMIT`, `z_drop_threshold`). A
turn runs between two arc ends on the same level set near one wall, so
that needs a Z step of the level floor inside 3.2 mm. The plain test
asserts that every turn move is at the level.

**Links: no, before C22.** C12 wrote each 2D `Link` as a `Cut`
(`links_as_cuts`) and the lift then classified its points:

- A return through the band just cut (one-way, C14) or a zigzag arc link
  runs where the 2D plan and the 3D stamp have both cleared the disc. The
  disc read finds no stock 0.2 mm above the point, so every point is
  "air".
- An air run is kept as a cut only when it is shorter than
  `air_run_crossover_mm` (C:1870). At the C12 fixture's feeds that is
  5.125 mm at Z 7 and 6.375 mm at Z 4.5 (C18 §3.2). A longer run becomes
  `Rapid(end_pt)` (C:2950, C:2977): an entry.
- That entry is a retract, a rapid and a plunge, or (with a stock proof)
  the F-038b keep-down link. That link ends at the entry's rapid floor
  (`try_emit_stay_down_link` step (c), PA:1333; `descent_floor`,
  PA:1575), and the entry style then feeds down from there to the level.
  This is C21's finding. So a link that the 2D plan cuts at depth through
  cleared stock became, in 3D, a lift and an entry that the 2D plan does
  not have.
- Which links: the one-way return is one `Link` per arc vertex (about one
  cell, 0.5 mm), and each became its own `Cut`. A 2-point air run of
  0.5 mm is under the crossover, so the C12 lift re-promoted it: the
  returns stayed at depth. The longer links became entries: a zigzag arc
  link, a keep-down link of the agent fallback or the cleanup, and the
  frontier links of the field. D12/D13 count 1469 entry samples on
  `geodesic_curv` against 291 on AgentSearch (the C12 summary line). That
  is consistent with this mechanism; the run decides how much of it is.

**The fix (research arm only).** `HeldLinks` (R:7529) takes each `Link`
after a known position as a `Cut` from the cutter position (subdivided at
the planner cell, as `links_as_cuts` did) and marks it held. In
`clear_one_region` a held cut is all engaged (`engaged.fill(true)`,
C:2822-2826), so the air demotion never makes it an entry. The large-Z
split stays (the terrain safety rule). The conversion moved from
`plan_one_outline` to `clear_one_region`, because the lift is where the
mark is needed. The F-038 group filter ran after `links_as_cuts` and
counted link length as cut length. It now counts `link_length` of the
group under `WithinPassLoad` (C:2433-2437), so the filter decides as it
did under C12.

**Why the fix adds no load.** The fix changes only runs that the disc read
called air, that is, runs whose whole cutter disc stands on cleared stock.
Such a move cuts nothing. Its stamp changes no cell. The entry it replaces
plunged through air too. So the change is time only: fewer entries, and a
feed move where there was a lift, a rapid and a plunge.

**What C22 did not change.** The remaining 3D entries of the arm (the 2D
re-entries, the lift's air runs inside a clearing cut, the large-Z splits)
still go through the shipped entry path, so C21's keep-down-link-to-the-
rapid-floor behaviour stays for them. That is shipped code; a change there
is the lead's.

## 4. Shipped neutrality (byte parity by construction)

- Without `research`: `plan_one_outline` calls `slice_cleanup` (the moved
  match, the same three calls with the same arguments), there is no
  `HeldLinks`, `held` is `false`, and `research_turn_feeds` returns the
  toolpath unchanged. `output_and_turns` reads `result.segments` by
  reference instead of moving it; the toolpath is the same.
- With `research` and a shipped arm: `research_slice_segments` gives
  `None`, the turn list stays empty, so the shipped cleanup runs;
  `held_links` is `None` (`keep_down_links` is `RetractedByCaller`); the
  F-038 link term is gated by `WithinPassLoad`; `SliceTurns` stays empty,
  so `apply_slice_turn_feeds` writes nothing.
- The C12 arms `geodesic_curv` and `geodesic_curv_split` have no turns
  (`ArcLink::Nearest`), so they keep the C12 cleanup. Only the held links
  change their output (§3).

## 5. Predictions (P)

- P1: `check` and both clippy configs are green. Risks: the `let` chains in
  `apply_slice_turn_feeds`; the cfg'd `let (seg, held)` shadow in
  `clear_one_region`; the private alias `ResearchTurns` in the private
  `output_and_turns`; `let held = false;` without `research` (no lint is
  known to fire on it).
- P2: `adaptive3d_emission_byte_parity` green with and without
  `research`; the 13 adaptive3d sentries green.
- P3: the plain test passes: both modes plan turns (C14/C16/C20 plan 50 to
  59 turns on the 2D island pocket), `unfed` 0, at least one reduced fed
  turn (2D factor quartiles 0.37 to 0.52), every turn move at Z 4.5.
  Confidence: medium. The risks: the 3D RDP (`cut_simplify_tolerance`,
  0.1 mm here, against the 2D emitter's 1 µm) can drop the vertex of a
  short turn. A dropped turn with factor 1 counts as `full_feed_unmatched`
  (allowed); one with factor < 1 counts as unfed and fails the test. That
  failure is real evidence, not a test defect. The run time is one level
  of two plans in debug: P about 30 to 90 s.
- P4: heavy test: `geodesic_curv` and `geodesic_curv_split` have fewer
  `entry` samples than D13 (1469, 1064) because of the held links, and more
  `judged` (fewer plunges, so fewer `departing`). Over the exact bound: 0
  in both (the held links cut air). The two new modes: `turn` samples > 0,
  `turn_faults` empty, 0 over the exact bound. Confidence: medium; the
  class (iv) wraps of C18 that had no cause can still show.
- P5: on the fixtures (§7) the arm has fewer entries than C12's arms would
  have, and the one-way column more keep-down feed travel than zigzag (as
  in 2D). Whether it beats AgentSearch on cycle time is open: the perimeter
  sweep, the waterline cleanup and the agent fallback are shared, and the
  turn feeds lower the feed on each turn.
- P6: the turn readout costs one extra `pass_load_readout` per slice piece
  with turns (the 2D harness does the same per piece). On rivmap100-sized
  jobs that is seconds to a minute in release; the run measures it.

## 6. Verifier commands

From the worktree, through the lane script, each with a `timeout`.

```sh
cd /home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head
timeout 120 scripts/cargo_lane.sh fmt --all -- --check
timeout 900 scripts/cargo_lane.sh check -p rs_cam_core --all-targets
timeout 900 scripts/cargo_lane.sh check -p rs_cam_core --all-targets --features research,test-support
# C22: the plain turn test (runs by default), then the heavy oracle (about 12 min, operator's go-ahead on the PC)
timeout 1800 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive3d_geodesic_arm_c12 -- --nocapture
timeout 3600 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive3d_geodesic_arm_c12 -- --ignored --nocapture
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features test-support --test every_ignore_states_its_kind_f152
# shipped neutrality: byte parity without and with research (env var unset)
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features test-support --test adaptive3d_emission_byte_parity
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive3d_emission_byte_parity
# the adaptive3d sentries (adaptive3d/CLAUDE.md), without research
for t in adaptive3d_boundary_clear_parity adaptive3d_keep_down_link_f038b \
  adaptive3d_entry_stock_aware adaptive3d_entry_coalescing_f038 agent_search_coverage \
  adaptive3d_subtool_channel_gouge adaptive3d_global_gate_drapes_below_floor \
  adaptive3d_plan_matches_merged_path_g_plansimgap adaptive3d_planner_never_ahead_of_emitted_path \
  adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom \
  adaptive3d_plans_every_slice_piece_g_adapt3dpieces adaptive3d_by_area_cells_confine_jobs \
  adaptive3d_by_area_matches_global_stock; do
  timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features test-support --test "$t" || echo "FAIL $t"
done
# the research kernel (C22 changes only the C12 block; C23's edits share the file)
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --lib adaptive::research_geodesic
timeout 900 scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --lib adaptive::research_geodesic -- --ignored
# both clippy configs, and the research one alone
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets -- -D warnings
timeout 1800 scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features research,test-support -- -D warnings
timeout 3600 scripts/cargo_lane.sh clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings
```

The `for` list is literal words (zsh does not split a variable). The
compute adapter (`compute/execute/rough_3d.rs`) is not changed, so the viz
and CLI builds see the same public `Adaptive3dOutput`.

Read the heavy test's lines in this order: per mode the `summary` line
(entry, judged, departing, turn), the `turns:` line (planned, cut, reduced,
lowest factor, longest against 3.207, fed / unfed / full feed with no
move), then any `screened:` line. Compare `entry` and `judged` of
`geodesic_curv` and `geodesic_curv_split` with D13 (§P4).

## 7. Fixture runs (prepare; do not run without the lead)

The originals stay untouched. A session writes each new project file once,
next to its original (the model paths are relative), with only the
clearing strategy changed:

```sh
P=/home/ricky/personal_repos/rs_cam/planning
C=$P/corne_case_analysis_2026-09-18
W=$P/multitool_2026-08-23
test -e $C/corne_job_corrected_c12_agent_search.toml || \
  sed 's/^clearing_strategy = "contour_parallel"$/clearing_strategy = "agent_search"/' \
    $C/corne_job_corrected.toml > $C/corne_job_corrected_c12_agent_search.toml
test -e $W/wanaka200_mt1_c12_agent_search.toml || \
  sed 's/^clearing_strategy = "contour_parallel"$/clearing_strategy = "agent_search"/' \
    $W/wanaka200_mt1.toml > $W/wanaka200_mt1_c12_agent_search.toml
# each must differ from its original only on clearing_strategy lines: Corne 2 (ids 1 and 3), wanaka 2 (ids 1 and 5)
diff $C/corne_job_corrected.toml $C/corne_job_corrected_c12_agent_search.toml
diff $W/wanaka200_mt1.toml $W/wanaka200_mt1_c12_agent_search.toml
```

The rough ops and the skips:

| Project | Rough ops run | `--skip` |
|---|---|---|
| Corne | id 1 "3D Rough 1" (fresh stock), id 3 "3D Rough 1 (copy)" (from remaining stock: the rest rough) | `4` (Waterline 3) |
| wanaka200 | id 1 "2 Back Rough" (fresh stock), the op C12 named | `0,2,3,4,5,8,9,10` (id 5 "6 3D Rough (front)" is skipped, as in C12; drop `5` from the list to run it too) |

Both projects set `min_region_cut_length_mm = 15.0`, so the F-038 filter
and its link term are active. Both use `entry_style = "plunge"`.

Then three runs per project, with the CLI built with the core research
feature. The variable must be in the environment of the process that
plans; each CLI run is its own process (the variable is read once).

```sh
cd /home/ricky/personal_repos/rs_cam/.claude/worktrees/adaptive-head
B="scripts/cargo_lane.sh run --release -p rs_cam_cli --features rs_cam_core/research --"
O=/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase2/c22
CP=$C/corne_job_corrected_c12_agent_search.toml
WP=$W/wanaka200_mt1_c12_agent_search.toml
for arm in agent geodesic_local_oneway geodesic_local_zigzag; do
  if [ "$arm" = agent ]; then E=""; else E="RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm"; fi
  env $E timeout 3600 $B project $CP --output-dir $O/corne_$arm --skip 4 --summary 2> $O/corne_$arm.summary.txt
  env $E timeout 3600 $B project $WP --output-dir $O/wanaka_$arm --skip 0,2,3,4,5,8,9,10 --summary 2> $O/wanaka_$arm.summary.txt
  env $E timeout 3600 $B rough-score $CP --toolpath 1 > $O/corne_$arm.score1.json
  env $E timeout 3600 $B rough-score $CP --toolpath 3 > $O/corne_$arm.score3.json
  env $E timeout 3600 $B rough-score $WP --toolpath 1 > $O/wanaka_$arm.score1.json
done
```

`env` with an empty `$E` runs the control with no variable. A wrong value
logs `RS_CAM_RESEARCH_ADAPTIVE3D_ARM names no research arm; AgentSearch
stays` and runs the control: grep each summary file for that line before
reading it. The shipped baseline is the original file (`contour_parallel`),
as in C12 §8.

What the CLI prints, and where each number is:

- **Cycle time.** `project --summary`: the line `Toolpaths: N | Cutting:
  .. mm | Rapid: .. mm | Time: ..s` (whole project, from the cut trace;
  `NOT MEASURED` without one), and per toolpath the kinematics line under
  `[ ] #id name (adaptive3d) — N moves, ..`. `rough-score`: per toolpath
  `cut_summary.total_runtime_s` and `cut_summary.runtime_by_intent`
  (cutting, rapid, retract, entry), and `cutting_duty`.
- **Entries.** `rough-score` only: `counts.entry_runs` (one per entry, a
  helix or a ramp counted once), `counts.entry_runs_by_kind`,
  `counts.retract_runs`, `counts.rises_to_top_z`. `project --summary` does
  not print an entry count.
- **Collisions.** `project --summary`: per toolpath `N collisions` (or
  `collisions NOT CHECKED`, which is not 0), and the `Verdict:` line
  (`Collisions` / `RapidCollisions`).
- **Coverage and gouge.** `project --summary` prints the triage block:
  `Safety (n):`, `Act on (n):`, `Advisories:` and `Counts:`. A gouge or a
  missed-coverage finding is a line there. I did not find a line that
  prints `triage.coverage` itself in `rs_cam_cli/src/project.rs`: the CLI
  prints `session.triage()` (project.rs:1101), and A2
  (`P15_A2_coverage_wiring.md`) found `triage.coverage` empty on a path
  without the coverage evidence. The verifier confirms where
  `triage.coverage` stands for these runs: in `summary.json` /
  `rough-score`'s `project_diagnostics`, or through the MCP
  `run_simulation` triage with a GUI built from HEAD with the research
  feature and started with the variable (the root `CLAUDE.md` HEAD check
  applies).
- **Feed modulation.** Both commands run the adaptive feed modulation by
  default (`Adaptive feed modulation:` line). It can rewrite the turn
  feeds on the emitted toolpath. The planner-level turn feed is what the
  C22 tests prove; a run with `--no-adaptive-feed-modulation` shows the
  turn feeds as planned.

## 8. What C22 could not do

- No build, test or clippy run (by the brief).
- The turn feed rides on the toolpath as a lower `Linear` feed. A later
  dressup that rewrites feeds (feed modulation, feed optimisation) is not
  turn-aware.
- With a corner radius (`min_cutting_radius > 0`) the blend at each turn
  end keeps the full feed, as in 2D. The two fixtures set 0.
- `apply_slice_turn_feeds` calls `holding_move` once per turn with no move
  on it, and that scans the whole toolpath. A run with many such turns is
  slower; the count says how many.

## 9. D14 fix: the turn feed survives the segment merge (2026-10-25)

D14 (`phase2/d14_c12_plain_output.txt`, `phase2/d14_c12_test_output.txt`)
found two faults. No cargo ran for this fix; rustfmt is green.

**Fault 1: 3 turns unfed (plain test and heavy test).** The cause is my
P3 risk. The emitter and the planner stamp simplify each `Cut` with the
ONE RDP rule (`Adaptive3dGeometry::chord_tolerance`). A 2D `Cut` holds a
turn and the steps next to it. Turns 42-45 run end to end on
y = -17.8757, so the RDP drops the shared vertices, and the merged move
lies on no single turn.

Fix: `research_geodesic::split_at_turns` (R:7839, in the C12 block).
`plan_one_outline` calls it on the F-038-filtered plan when the turn list
is not empty (C:2475). It splits each `Cut` where a run of steps on one
turn starts or ends. So each turn is a `Cut` of its own, and the steps
between two turns are one `Cut`. The RDP keeps the two ends of every
`Cut`, so no emitted move can join a turn move with a move of another turn
or of no turn. Properties:

- No second RDP. The ONE function still runs, per `Cut`, in the emitter
  and in the planner mirror.
- G-PLANSIMGAP holds. The split is on the 2D plan, before the lift, so the
  planner stamps exactly the segments the emitter emits (merge before
  stamp, as before).
- The points and their order do not change. Only the RDP can no longer
  merge across a split point.
- Only research plans with turns are split. The shipped arms and
  `geodesic_curv` / `geodesic_curv_split` are untouched.

**Fault 2: 6 turns "counted fed, and no move carries factor" (heavy test
only).** This is a fault of the test's attribution, not of the feed:

- `turn_moves` gave each move only to the FIRST record it lies on
  (`position`).
- The pocket walls are vertical, so Z 7 and Z 4.5 plan the same turns.
  The faulted turns 42-45, 56 and 61 have indices past the first level's
  43 turns.
- `apply_slice_turn_feeds` writes the feed on the move for every record
  it lies on (XY rule, `min` of the feeds) and marks each record fed. The
  test then credited the move only to the first-level record.

The plain test (one level) shows no such fault, which agrees with this
cause.

Test change (`tests/adaptive3d_geodesic_arm_c12.rs`): `turn_moves` (T:303)
returns every record a move lies on. `turn_faults` (T:342) requires the
move's feed to be at most the SMALLEST `F · factor` of those records, and
credits it to each of them. This is not looser: each record still needs
its own move at or under its own reduced feed, and a move on two records
is now held to the lower feed (before, only to the first record's).

**P (D15).** Plain test: `unfed 0`, and the run passes. Heavy test: no
`turn_faults` in modes 3 and 4. Risk: a turn whose 2D steps the lift
demotes or splits (air, large Z) still has no move; the count then says
so.

## 10. C26: the chord tolerance of the research arm (G-RDPLOAD) and keep-down links tagged as links (2026-10-25)

Base 4cceca914 (worktree `adaptive-head`). No cargo ran and nothing is
committed. `rustfmt --edition 2024 --check` is green on the four changed
Rust files. Inputs: `P2_C2_geodesic_fixes.md` §C24, and G-RDPLOAD in
`DEFECTS.md`. In `research_geodesic.rs` only the C12 block changed; C23's
functions are not touched.

### (1) G-RDPLOAD: a chord tolerance from the load margin

**The rule.** For the research arm:

`t_arm = min(chord_tolerance(tolerance, segment_merge), c·D − min(s, D))`

with `MIN_EMITTED_SEGMENT_MM` as the floor of the margin.

- `c·D` is the replay's own ceiling: `PassLoad::swept_width(s, R,
  cell).ceiling` times D.
- `min(s, D)` is the swept width of a straight pass at the level step.
- So the margin is the room between the straight pass and `c·D`.
- A chord a sagitta `h` inside its walked arc leaves `h` that the 2D plan
  held cut. The next pass meets `s + h` at mid-chord, so `h ≤ c·D − s`
  keeps that pass at or under `c·D`.
- No new constant.

| Fixture | s, R | c·D | margin | chord_tolerance | t_arm | chord at r 60 (2√(2rh − h²)) |
|---|---|---|---|---|---|---|
| Corne | 1.2, 3 | 1.3132 | 0.1132 | max(0.1, 0.3) = 0.3 | **0.1132** | 11.98 → 7.37 mm |
| C12 test | 2.0, 3 | 2.1759 | 0.1759 | 0.1 (no merge) | 0.1 (unchanged) | — |

**Where.** `Adaptive3dParams::emission_chord_tolerance` (`adaptive3d/mod.rs`,
new `impl Adaptive3dParams`):

- Shipped arms: `geometry.cut_simplify_tolerance()`, unchanged.
- Research arm (`ClearingStrategy3d::Geodesic`, cfg `research`): that
  value `.min(research_geodesic::slice_load_margin(s, R, tolerance))`.
  `slice_load_margin` is in the C12 block.
- The rule stays ONE function, `Adaptive3dGeometry::chord_tolerance`. The
  cap only lowers it, so the model gouge guard, which reads the uncapped
  rule (G-ROUGHGOUGE), stays safe.

**Readers.** All four readers of the old value move to the accessor, so
the planner and the emitter keep one value (G-PLANSIMGAP):

- `adaptive3d/path.rs`: `ClearZLevelContext::cut_tolerance` (planner
  stamp), the two `waterline_cleanup` calls, and the emitter's
  `simplify_path_3d` in `segments_to_toolpath`.
- `adaptive3d/CLAUDE.md` names the accessor; the file is still 40 lines.

**Limit.** The margin is the straight-pass room. Inside the curvature
law's radius ρ* (25.45 mm on Corne) the law spaces the passes to read
`c·D` already, so a sagitta up to `t_arm` can still read over by up to
`t_arm` there. Chords at small ρ are short for a given sagitta. The
exact route is the lead's other option, the 2D rule run again on the
simplified path; it would turn a sagitta into a refused step (a re-entry)
instead of a smaller chord, so I did not take it.

**The shipped arms (estimate, not changed):**

- AgentSearch on Corne uses the same 0.3 mm chord tolerance. Its passes
  are 2.55 mm apart (C24: 2.1 × s, 20 % of its cut over the bound), so
  the spacing already exceeds `c·D` by about 1.24 mm. A sagitta of up to
  0.3 mm adds to that; it is second to the spacing. Its 20 % would fall
  only a little with a capped tolerance. The run would have to measure it.
- The 2D shipped emission (`WithinPassLoad`) keeps the walked line: its
  RDP tolerance is `MIN_EMITTED_SEGMENT_MM`, 1 µm (`adaptive/path.rs`
  `segments_to_toolpath`). So G-RDPLOAD does not apply there. Only the
  historical branch (`RetractedByCaller`, `simplify_path(params.tolerance)`)
  leaves sagittas up to the operation tolerance, and no shipped 2D emission
  takes that branch under the 2D rule.

### (2) The 2D keep-down links are emitted as 3D keep-down links

**Cause.** C22 held each 2D `Link` as a `Cut`, which the emitter tags
`MoveIntent::ClearingCut`. The semantic trace and the pages read those
moves as cut: C24 counts 1940 mm at Corne Z 5.00, 22 % of the one-way cut
length.

**Fix.** `HeldLinks` (C12 block) now only flags a `Link` that comes after
a known position; it no longer converts it. `clear_one_region`
(`adaptive3d/clearing.rs`) emits each flagged link as
`Adaptive3dSegment::Link(lift(end))` and goes to the next segment.

- That is the existing 3D keep-down link: the emitter drapes it to the
  leave (`drape_path_to_leave`, sampled at ≤ R, no RDP) and tags each move
  `MoveIntent::Linking`.
- `spans_from_move_intents` makes that a `LinkBridge` span ("link").
- The planner stamp mirrors it (`stamp_emitted_segment`, the `Link` arm,
  G-PHANTOMSTAMP), and `LevelSink::emit` commits a pending entry before it
  and clips it (`centre_clip`), as for every link.
- The air fill of C22 (`engaged.fill(true)`) is gone. The cut path of
  `clear_one_region` is now the shipped text again.
- The F-038 filter still counts link length (`link_length`), so its
  decisions do not change.

What is lost: the lift's large-Z split no longer applies to a held link.
The 2D replay holds a link inside the tool-centre region of the level,
where the floor is the level. The drape follows the surface plus the
leave, as for every 3D keep-down link.

### Predictions (P)

- **Byte parity** (`adaptive3d_emission_byte_parity`, with and without
  `research`): green. For a shipped arm `emission_chord_tolerance` returns
  `cut_simplify_tolerance()` exactly, and `held_links` is `None`.
- **Heavy C12 test, 0 over: holds.**
  - The fixture has no merge, so `t_arm` = 0.1, as before: (1) changes
    nothing there.
  - (2) changes the held links from `ClearingCut` to `Linking`. The oracle
    does not exempt `Linking` (only the entry intents), so it still judges
    those samples. They cut the band just cut (the 2D replay held each
    link within the load), so they add no sample over.
  - The geometry is the same straight line; the drape now samples at R
    instead of the planner cell, so the move counts differ.
  - The turn rule is unchanged (turns are cuts, not links).
- **Plain C12 test:** unchanged verdict. Turn moves stay at the level, and
  the turn split (D14) is unchanged.
- **Corne D-FIX2 sim share over the bound (zigzag 4.6 %):**
  - (1) removes the mid-chord excess on the wraps outside ρ*: mid-chord
    reads fall from up to 1.49 mm to at most 1.313 mm.
  - (2) moves the link length out of the cut class. The links cut air,
    so the over count does not change but the cut denominator falls (by
    the link share: about 22 % of the one-way cut at Z 5.00, less for
    zigzag).
  - Net: zigzag 4.6 % → about 1.5 to 3 %. The rest is the ρ* limit above
    and the shipped waterline / perimeter passes. One-way: the share can
    read higher than before on the smaller denominator, unless (1) removes
    most of the over samples. Confidence: low; the D-FIX2 run decides.
- **Cycle time:** (1) gives about 1.6 times the moves on the large wraps
  (7.4 mm chords for 11.8 mm). That adds time only through the per-move
  acceleration model. (2) gives the same path length.

### Verifier commands (C26)

The §6 list. The changed lines are the C12 test (plain and `--ignored`),
byte parity both ways, the 13 adaptive3d sentries, and both clippy
configs. Also run `adaptive3d_plan_matches_merged_path_g_plansimgap`:
the emitter and the planner now read `emission_chord_tolerance`.

## 10b. C26b: the gouge at Corne X 55.935 Y -15.766 is a held link (2026-10-25)

D16c (`phase3/fix3/corne/`, at 6ce373922) refused the export of
`geodesic_local_zigzag` and `geodesic_local_zigzag_v2`. The deepest point
is 1.450 mm below the leave at X 55.935 Y -15.766: the tip is at Z 2.250
and the limit is Z 3.700. At 4cceca914 the export was clean.

### The move (from the sim samples in `simulation.json`, zigzag_v2)

| Move | From → to | What it is |
|---|---|---|
| 9772–9774 | a curve at Z 4.0 to (55.51, -15.84) | a cut |
| 9775–9777 | rise to Z 4.5, traverse to (54.868, -15.7655), plunge to Z 0.5 | the shipped F-038b keep-down entry (trace item 814, "Pass 806 keep-down link", moves 9775–9777) |
| **9778** | (54.868, -15.7655, **0.5**) → (≈56.97, -15.7655, **≈4.0**) | **the gouge**: one straight move that climbs 3.5 mm over 2.1 mm. At x 55.935 it is at Z 2.25 (the sample line gives z = 0.5 + 1.64·(x − 54.868)) |
| 9779–9780 | (56.97 → 61.27, -15.7655) at Z 4.0 | collinear moves of 2.13 mm each |

Moves 9778–9780 are three equal steps of 2.13 mm on one straight line at
the constant y of the entry, with no RDP merge of the collinear 9779/9780.
That is the drape of a two-point 3D `Link` (`drape_path_to_leave` at ≤ R:
6.4 mm in 3 steps). A cut would have been RDP-merged. So it is **a C26
held link**: a 2D `Link` from the entry point to (61.27, -15.7655), which
C26 emitted as one `Adaptive3dSegment::Link`. The level is Z 0.5, and the
link crosses terrain whose surface plus leave is Z 4.0. Under C22 the same
link was a cut lifted at the planner cell. The lift's large-Z split
(slope 3.5 / 0.5 > `PLUNGE_SLOPE_LIMIT` 0.3) put a rapid at the wall, so
the toolpath retracted there. C26 removed that split for links. The
lead's suspicion is right.

### A real cut, not a side-of-wall reading

- At the move start (x 54.868) the tool stood at Z 0.5 within the limit.
- At x 55.935 the drop-cutter limit is Z 3.7. So within 1.07 mm of travel
  the model under the tool disc rose by more than 3 mm. The disc, at tip
  Z 2.25, reaches into that higher material by up to the 1.07 mm the
  centre moved toward it.
- The flat bottom of the end mill is below the material inside the disc:
  the cutter body overlaps the model.
- It is not only a side contact at exactly R: the move starts where the
  wall is outside the disc and drives the disc into it while it climbs.

### The fix (C26b; `clearing.rs` `clear_one_region`, and `HeldLinks` in the C12 block)

- `HeldLinks::next` now returns, for a held link, the path from the
  cutter position to the link end, subdivided at the planner cell.
  `HeldLinks` holds the cell again (`HeldLinks::new(params)`).
- `clear_one_region` lifts every point of that path (`lift`, the one
  level floor `level_floor_z`).
  - **All points at the level** (`p.z <= z_level`): one
    `Adaptive3dSegment::Link` to the lifted end, tagged `Linking`, as in
    C26. This keeps the tag for every link that stays down.
  - **Any point above the level** (the link crosses terrain): the C22
    path. It goes out as a `Cut` held at depth (`engaged.fill(true)` is
    back, under `held`). So the lift's large-Z split retracts it at the
    wall, as at 4cceca914. These moves are `ClearingCut` (untagged), and
    only such wall-crossing links take this path.
- Shipped arms: `held_links` is `None` and `held` is `false`, so the
  emission is byte-identical, as in C22.

### Predictions (P)

- Corne zigzag and zigzag_v2: the export is clean. The refused move
  becomes the C22 path (a retract at the wall), which exported cleanly at
  4cceca914. One-way: still clean.
- The link tag: most held links stay down, so most of the 1940 mm (Z 5.00
  one-way) stays tagged `Linking`. Only links that cross terrain above
  their level go back to `ClearingCut`.
- Heavy C12 test: unchanged from C26. On the pocket every link's lifted
  points are at the level (a flat floor; the 2D region is inset 2R from
  the walls), so every held link stays a `Link`. 0 over holds.
- Byte parity green (shipped paths unchanged).
- Residual risk: a link whose lifted points read the level from the
  heightmap but where the emitter's exact drop-cutter drape rises between
  samples. That drape then lifts the link, as it lifts every 3D keep-down
  link; the guard measures it.

### Files

`crates/rs_cam_core/src/adaptive3d/clearing.rs` and
`crates/rs_cam_core/src/adaptive/research_geodesic.rs` (the `HeldLinks`
item in the C12 block only; C23's uncommitted C28 edits are not touched).
No cargo, no commit; rustfmt is green.

## 10c. C29 build: the 3D arm's pick scored by the 3D stage (2026-10-25)

Built to C23's design (`P2_C2_geodesic_fixes.md` §C29, option (a)). Base
13667775d, worktree `adaptive-head`. No cargo ran and nothing is
committed. `rustfmt --edition 2024 --check` is green on the three changed
Rust files. In `research_geodesic.rs` only the C12 block changed.

### Changes

- **`adaptive3d/path.rs` (shipped emitter).** `try_emit_stay_down_link`
  is split. The new `pub(super) fn stay_down_link_z(from, to,
  descend_to_z, stock_ceiling, mesh, index, cutter, stay_down_mm,
  clearance, leave, safe_z) -> Option<f64>` holds steps 1–5 (the distance
  test, the 10-sample mesh probe, `link_z`, the `safe_z` and
  cutter-length guards), with the same arithmetic in the same order. The
  emitter calls it and then emits steps (a)–(c) as before; `xy_dist` is
  computed again with the same expression. `Adaptive3dSegment` gains
  `#[derive(Clone)]` (for the scratch copy of a pending entry). The
  `ClearZLevelContext` initialiser gains the cfg-research field
  `stay_down_clearance_mm: params.linking.stay_down_clearance_mm.max(0.0)`,
  the value the emitter reads.
- **`adaptive3d/clearing.rs`.**
  - `clear_one_region` keeps the forecast and the perimeter sweep, which
    both variants share. The body after them is now
    `plan_and_emit_region(ctx, plan, emission, region_polygon,
    region_idx, strategy, piece_debug, cancel)`: the 2D plan per outline,
    the eng lookup, the held links and the lift loop, moved verbatim.
  - `plan_one_outline` and `research_slice_segments` take the slice
    generator `strategy` instead of reading `ctx.clearing_strategy`; the
    shipped call passes `ctx.clearing_strategy`.
  - The lift closure is now `lift_to_level`, with the same arithmetic.
  - Research only:
    - `research_pick_variants` gives `(Geodesic(A), Geodesic(B))` when
      the options have `ArcPick::Replay`.
    - `RegionScratch::run` copies the level state (the planner stock,
      the cursor, the level's segment tail from the pending entry, the
      metrics, the counters) and runs `plan_and_emit_region` on the
      copy, with no debug context.
    - `RegionScratch::score` counts plunges and keep-down length with
      the emitter's own test (`stay_down_link_z` on the proof's `from`,
      `drape_point` of the entry and `descent_start_z`).
    - `pick_region_plan` keeps fewer plunges; on a tie, less keep-down;
      on a tie of both, A. It commits the winner's copy into the level
      state, drops the loser's turn records from `ctx.research_turns`,
      and prints one `info` line per region: "C29 pick (3D score)",
      with z, region, plunges_a, keep_down_a_mm, plunges_b,
      keep_down_b_mm and kept.
- **`research_geodesic.rs`, C12 block.** `slice_variants(options) ->
  Option<(GeodesicOptions, GeodesicOptions)>` is
  `options.pick_variants()` when `pick == ArcPick::Replay`. Each variant
  then plans through `slice_segments` with the pick off, so the 2D pick
  of `geodesic_segments` does not run in 3D. The 2D harness keeps its 2D
  pick.

**The design's "both plans" door.** `slice_variants` gives the two
generators, not two plans. Each variant's 2D plan is made inside its own
scratch run of `plan_and_emit_region` (outline by outline, with the
cleanup, F-038, the turn split and the turn records). So the region body
stays one function that takes a generator, and no plan is made a third
time.

### Byte parity (by construction)

- Shipped arms: `research_pick_variants` does not exist without
  `research`, and with it it gives `None` for every shipped arm.
- The moved body is the same text and gets the same arguments.
- `stay_down_link_z` returns `None` exactly where the emitter returned
  `false`, with the same `link_z`.
- So `adaptive3d_emission_byte_parity` stays green with and without
  `research`.

### Memory (Corne)

The planner stock is the Z grid at the planner cell (`max(R/6, tol)` =
0.5 mm) over the stock box (152.8 × 111.2 mm, the sim artefact box): about
306 × 223 = 68 k rays.

- Each ray is a `SmallVec<[DexelSegment; 1]>` (about 24 B) plus a 4 B
  `conservative_top`, so about 1.9 MB per copy (more if the planner stock
  has its X/Y grids: about +0.7 MB at 46 Z cells).
- Two copies per region, alive with the level's own, so about 4–5 MB
  extra at the peak. They are freed at the end of the region.
- The copies are per region, not per level, as the brief asks.

This is an estimate from the types and the box; the run measures it.

### Predictions (P)

| Case | Kept | Plunges | Cycle |
|---|---|---|---|
| Corne "3D Rough 1", `geodesic_local_zigzag_pick` | A at every level (`info`: A 10/6/6/34 against B 14/8/8/37, as D17's spans) | 56 (= zigzag) | about 2607 s (D17 zigzag) |
| C12 plain test, `geodesic_local_zigzag_pick` | B | 48 (as zigzag_v2) | — |

- The `info` plunges per region equal the trace's plunge spans of that
  level, less the shared ones: the first entry, whose proof cannot match;
  on Corne Z 0.50, the 23 small-region plunges are counted in each
  region's own line.
- Planning time of the pick arm: about 2 × one variant's planning time
  plus the copies (Corne: about 20–25 s).
- The approximation that can make the score wrong: the emitter's 0.05 mm
  `from` match. The per-region lines show it if it happens.

## 10d. C29 on Corne (D18): which plunges the pick scores (2026-10-25)

Data: `phase3/fix5/corne/geodesic_local_zigzag_pick/` (D18 at f32401326):
`stdout.txt` (the C29 lines), `tp_1_3D_Rough_1.json` (the debug spans and
the semantic entries). No code changed; no cargo.

### (1) Region 1 of Z 11, 5 and 0.5: the pick does not run there

The debug spans `agent2d_region "region 1/2 @ Z 11.000"` (and the same at
Z 5 and Z 0.5) carry `forecast_cut_length_mm 0.0` and `skipped_f038 1`.
The F-038 forecast in `clear_one_region` found less than
`min_region_cut_length_mm` (15 mm) of cut and returned before the
perimeter sweep. That is the only early return before the pick. So the
region has no perimeter, no 2D plan, no entries and no pick line. The two
variants are not identical there: the region is never planned. The
forecast reads the region polygon and an agent dry run, not the variant,
so the skip is the same for A and B.

### (2) The trace plunges that are outside the score

The semantic trace of D18 counts **7 / 7 / 7 / 35** plunge entries at
Z 17 / 11 / 5 / 0.5. These are the entries with style "plunge entry"; the
lead's 10/6/6/34 are D17's span counts. The C29 lines score
**4 / 5 / 5 / 4**. I ordered each level's plunges by move index and
placed them by phase:

| Level | Before the body | Body (scored) | After the body: waterline cleanup | Sum |
|---|---|---|---|---|
| Z 17 | 1: move 0, (20.0, -106.3): the op's first entry, the perimeter sweep's start | **4**: moves 127, 1783, 3424, 3524 | 2: moves 3889, 3969 (2 contours) | 7 |
| Z 11 | 0 (the perimeter entry is a keep-down link) | **5**: 4172, 5551, 5704, 5714, 5739 | 2: 5747, 5848 | 7 |
| Z 5 | 0 | **5**: 6051, 7435, 7588, 7598, 7627 | 2: 7645, 7751 | 7 |
| Z 0.5 | 0 | **4**: 7980, 10166, 10401, 10409 | 31: 10511 … (31 contours; the grid of small regions D17 saw) | 35 |

Each level's waterline starts at (20.3, -106.2) and goes on at
(26.3, -96.6), as `waterline_cleanup` traces its contours after the
regions.

- The scored plunges equal the trace's body plunges at every level
  (4/5/5/4). So the score's one approximation, the 0.05 mm `from` match,
  held everywhere.
- Outside the score:
  - the op's first entry (the perimeter sweep of Z 17);
  - every waterline-cleanup entry (2/2/2/31);
  - the F-038-skipped regions (they have none).

### (3) Can any of those differ between A and B?

| Outside item | Same for A and B? | Why |
|---|---|---|
| F-038 forecast skip | yes | It reads the region polygon and an agent dry run, before the pick. |
| Level's first entry / perimeter sweep entries | yes | They are emitted on the real state before the pick, from the cursor and stock of the regions before. |
| **The first entry after the body** (the next region's perimeter entry, or the waterline's first entry for the last region) | **no** | Its keep-down test reads the cursor where the body ended (`plan_entry`: the proof needs the jump ≤ `stay_down_mm`; `corridor_ceiling_z` reads the stock) and the emitter's `stay_down_link_z`. A and B end at different points. It can be a plunge after A and a link after B, or the other way. At most 1 plunge per region. |
| Waterline entries after the first | mostly yes, not exactly | The contours come from the mesh and Z, so they are the same. But the lift demotes a waterline run through air to an entry, and the rapid floors read the stock the body left. A sliver near the wall that one variant leaves and the other clears can add or remove an air split. Expected rare: the perimeter sweep and both bodies clear the same band. |
| The next level | yes, at the second order | The next level's regions and 2D plans read the material grid at that level, which the body of this level does not change below its own floor. Its first entry reads the cursor where this level's waterline ended, and the waterline is the same for both. |

**The real gap.** The score leaves out the exit transition of the region
(0 or 1 plunge) and, at the second order, the waterline's air splits.

- The pick can choose wrongly only when the two bodies differ by at most
  1 plunge (or tie on plunges and the exit flips the keep-down order).
- On D18 the margins were 4, 2, 2 and 3 plunges (A 4/5/5/4 against B
  8/7/7/7), so D18's choice of A is safe.
- The C12 plain test has one region per level, and D16a's margin between
  the two variants was 20 entries (68 against 48), so it is not at risk
  either.

**The proposed fix (not built).** Score the exit too. For each variant,
after its body, probe one keep-down link from the variant's scratch
cursor to the next entry point, with the same code the sink and the
emitter use:

- `plan_entry` on the scratch stock gives the proof and the floor;
  `stay_down_link_z` on the draped point decides the link.
- A refused link adds 1 to that variant's plunges.
- The next entry point is known before the region runs:
  - another region follows in the level: the first vertex of its inset
    exterior `offset_polygon(next_region, R + 0.25)` (the perimeter
    sweep's own start; `clear_z_level_agent_2d_slice` has the ordered
    region list);
  - the last region of the level: the first point of the level's first
    waterline contour. `waterline_cleanup` traces its contours from the
    mesh and Z only, so `path.rs` can trace them before the level's
    regions and pass the first start point in. That is one reordering in
    the shipped level loop, so byte parity is the check. A
    research-only alternative: compute the same first contour inside
    the pick, at the cost of one extra waterline trace per level.
- No new constant. The waterline's air splits stay outside the score; they
  are second order, and an exact score would need the pick at level scope
  (the waterline run twice and stock copies per level). I do not
  recommend that.

## 11. C31 design: open each region of the research arm under the 2D rule (2026-10-25)

DESIGN ONLY. No Rust changed, no cargo, no commit. The operator ruled:
replace the perimeter sweep in the geodesic research arm only; the shipped
output stays byte-identical. Inputs: C30 (`P3_C18_3d_oracle.md`), D18
(`phase3/fix5/corne/geodesic_local_zigzag_pick/`). Measurement script:
`phase3/c31/band.py`, output `phase3/c31/band_output.txt`
(`python3 -I band.py ../fix5/corne/geodesic_local_zigzag_pick`, a few
minutes; it uses C30's phases and stamps discs of R on a 0.1 mm lattice).

### 11.1 What the region polygon and the sweep are (code, read)

- **The region.** `build_material_bool_grid` marks a cell when the stock
  stands above `max(drop-cutter surface + leave, z_level)`. The surface is
  the drop-cutter heightmap (tool-centre Z). So the region's boundary is
  of one of two kinds:
  - **At a wall that stands above the level:** the boundary is the
    tool-centre limit, about R (+ leave) from the wall. That is the
    waterline's own locus.
  - **At the stock edge, or where an earlier pass cut to the floor:** the
    boundary is where the material ends (material space).
- **The 2D planner insets the region again.** The outline is smoothed
  (`SIMPLIFY_INSET` 0.3 + RDP 0.3), and the 2D planner treats it as a
  material outline and insets it by R. So its cutter centres stay at least
  2R + 0.3…0.6 from a wall, and their edges at least R + 0.3…0.6.
- **The sweep.** One loop per inset ring at `R + 0.25` inside the region
  (`PERIMETER_INSET_MARGIN_MM`), cut BEFORE the 2D plan, at the full level
  depth, through fresh stock: a full slot (C30: exact 0.95–1.00, 1026 mm
  over 0.471). Its centre locus is the 2D plan's own outermost locus,
  0.05–0.35 mm further out. Its own comment says why it exists: the agent
  walk "doesn't reliably sweep the full machinable boundary". The 0.25 mm
  margin keeps it inside the containment boundary (O5b).
- **The waterline cleanup** (after the regions) runs at the wall locus, so
  at a wall it meets the band from the leave to the edge of the sweep or of
  the 2D plan: R + 0.25 or more. That is a half slot (G-WATERLOAD).

### 11.2 Measured: what fails if the research arm skips the sweep (Corne D18)

Per level: the sweep's band (discs of R round its samples). The part of it
that the body does not cut is what the arm leaves without the sweep.

| Level | Sweep (mm) | Band cut by the sweep only (not the body) | ... and not the waterline | Sweep → nearest body sample (median / p95 / max mm) | Sweep → nearest waterline (median mm) |
|---|---|---|---|---|---|
| Z 17 | 456.8 | 33.1 mm² (mean 0.07 mm wide) | 15.4 mm² (0.03) | 0.12 / 0.23 / 1.32 | 0.51 |
| Z 11 | 374.8 | 184.1 mm² (0.49) | 0.0 | 0.47 / 1.40 / 5.48 | 3.28 |
| Z 5 | 374.8 | 159.4 mm² (0.43) | 0.0 | 0.32 / 1.39 / 5.48 | 3.28 |
| Z 0.5 | 374.8 | 148.9 mm² (0.40) | 144.5 mm² (0.39) | 0.20 / 1.30 / 5.48 | 7.90 |

What this means for each item the brief asks about:
- **The uncut band.** The body's outer pass runs 0.12–0.47 mm (median)
  inside the sweep locus, and up to 5.5 mm inside it at corners. Without
  the sweep a ring of mean width 0.4–0.5 mm (Z 11, 5, 0.5) stays, up to
  about 2.5 mm wide at the corners.
  - At Z 11 and Z 5 the waterline then cuts that ring too, so its load
    grows by the ring width.
  - At Z 0.5 nothing cuts it: 145 mm², 0.39 mm mean, standing up to the
    level depth.
  - So the plain skip (candidate 1 as it stands) leaves stock or loads the
    waterline. It is not enough.
- **The centre clip.** The sweep is the only producer at a fixed 0.25 mm
  margin. A replacement must keep its centres inside the region by the
  same margin (O5b).
- **Wall stock.** The band [leave, R + 0.25] from a wall is the
  waterline's in every case; the sweep does not touch it.
- **The waterline after it** (C30's 0.471 measure, split by the distance
  of each waterline sample to the sweep):

| Level | Waterline linear/arc | Over 0.471 at a planned region's wall (≤ R + 1.25 mm from the sweep) | Over 0.471 elsewhere |
|---|---|---|---|
| Z 17 | 446.0 mm | 11.4 | 0.0 |
| Z 11 | 663.1 mm | 71.4 | **169.2** |
| Z 5 | 663.5 mm | 49.1 | 6.9 |
| Z 0.5 | 418.9 mm | 2.4 | 6.6 |
| sum | | **134.3** | **182.7** |

"Elsewhere" is a waterline contour no clearing plan reached: in practice
the F-038-skipped region 1 of Z 11 / 5 / 0.5 (C22 §10d). Its first
contour starts at (20.3, -106.2), where the level's first waterline
entry is. The waterline traces every mesh contour at the Z (C30, the F9
note), so there it cuts a full slot alone.

**The C12 fixture.** I could not measure it (no artefact; a run is
needed). From the code: the pocket and the island are vertical walls, so
both of their boundaries are of the wall kind. The heavy oracle exempts
the sweep's distance band and the waterline by tag, so it would hide the
same two slots.

### 11.3 The candidates

1. **Skip the sweep; the field's outer wraps open the band.** Measured
   above: not enough as it stands. The 2D plan's outer reach is the
   smoothed outline inset by R, 0.05–0.35 mm short of the sweep and short
   by up to 5.5 mm at corners.
2. **A ring at the sweep locus, cut from inside, after the body.** Its
   engagement is the gap above:
   - median 0.2–0.5 mm, under c·D 1.313;
   - p95 1.3–1.4 mm, at the ceiling;
   - corners up to about R + 2.5, over it.
   To hold the corners it would need inner rings (mostly air, and the lift
   turns long air runs into entries) or a 3D load reading per step. It
   also does nothing for the waterline's half slot.
3. **RECOMMENDED: plan the research arm's slice on the region's own
   tool-centre limit.** This is candidate 1, made complete.
   - For the research arm only, `planner_outlines` gives the 2D planner
     the region grown outward by `R − m`, where `m` is
     `PERIMETER_INSET_MARGIN_MM`. The grow is the one `inset_piece_outlines`
     already uses: `OffsetRingSet::from_polygon(..).offset(-(R − m))`,
     arcs kept, untoleranced flattening. No smoothing inset and no RDP.
     The 2D tool-centre region (the grown outline inset by R) is then the
     region inset by `m`: the sweep's own margin, kept for O5b.
   - The geodesic field and the 2D cleanup (residue contours, wall pass,
     mop) then open the whole band, out to `m` from the region boundary,
     **under the 2D pass-load rule**: the replay holds every step, and the
     wall pass of the cleanup is the ring of candidate 2, now cut by the
     planner that measures it. The research arm emits no perimeter sweep.
   - At a wall the 2D grid now holds the real material: the grown band
     is the stock between the region boundary and the wall. So the
     waterline meets only `m` plus one planner cell
     ((0.25 + 0.5) / 6 = 0.125 D, under c 0.219 at s 1.2).
   - **The closing guard.** Growing and then insetting by R closes gaps of
     the region narrower than 2R: a tool centre could then be planned
     where the region has none (over a narrow ridge). Per region, compare
     the area of the 2D tool-centre region (the grown outline inset by R)
     with the region inset by `m`. If it is larger by more than one
     planner cell times the region's perimeter, plan that region as now
     (smoothing and sweep). That is the shipped path. No new constant:
     R, `m` and the planner cell are existing values.
   - **The cost at a material boundary** (the stock edge; Corne Z 17).
     There the 2D grid assumes material for `R − m` beyond the real edge,
     so it plans about `(R − m)/s` more wraps that cut partly air. It is
     pessimistic, not unsafe: the real engagement is less than the planned
     one. A later refinement can grow only along wall stretches (boundary
     points within R + leave + one cell of a mesh waterline contour); I do
     not propose it now.
4. **Part 2: the waterline as a skim held to the ceiling, else to the
   stock left** (the operator's earlier ask; research arm only, in
   `waterline_cleanup`).
   - Before each waterline step, read the standing width ahead on the
     planner stock: the 3D form of the 2D swept width. In the disc of R at
     the step end, take the cells whose stock top is over `p.z +
     AIR_THRESHOLD_MM` (the lift's own threshold), and their extent across
     the travel direction, over D.
   - At or under the 2D ceiling `c` (`PassLoad::swept_width`), the step is
     cut. Over it, the step is not cut: the run ends, and the stock there
     is left, counted and printed per level (length and the largest width
     left).
   - After part 1, every planned wall meets ≤ 0.125 D, so the skim cuts
     there. What it leaves is the waterline of the F-038-skipped regions:
     F-038's own rule ("material left for finishing passes") extended to
     the wall of a region it did not open. The next op (the rest rough on
     Corne) and the finish see it.
   - No new constant.

### 11.4 Predictions (P; C30's measure, 0.471, Corne "3D Rough 1", zigzag_pick)

| Item | D18 | Part 1 (3.) | Parts 1 + 2 |
|---|---|---|---|
| Perimeter sweep over | 1026.1 mm | 0 (no sweep) | 0 |
| Waterline over, planned walls | 134.3 | ≈ 0 (meets ≤ 0.125 D) | ≈ 0 |
| Waterline over, elsewhere | 182.7 | 182.7 (unchanged) | 0 (left as stock instead: about 330 mm of wall at Z 11 / 5 / 0.5, measured by the run) |
| Departures (2D-exempt) and starter | 32.6 | about the same | about the same |
| **Total over** | **1375.8 (4.1 %)** | **≈ 215 (≈ 0.6 %)** | **≈ 35 (≈ 0.1 %)** |

- **Plunges (56 at D18).**
  - Part 1: −1, because Z 17 loses the sweep's entry, the op's first.
    The body's first entry is the starter plunge at every level already;
    at Z 11 / 5 / 0.5 the sweep's entry was a keep-down link.
  - Part 1 also lets the 2D replay re-enter in the new band. The pocket
    of D16a planned turns at walls without new entries, so I expect 0 to
    +6. Part 1: **55 to 61**.
  - Part 2 removes the waterline's entries on fully left contours (the
    first contour of Z 11 / 5 / 0.5: −3) and can add 2 per left span
    inside a contour. **About 52 to 60.** Low confidence.
- **Cycle (2607 s at D18).**
  - Removed: the sweep slots, about 1580 mm on `linear` plus their
    `helix` (draped) parts, at the modulated (slot) feed.
  - Added: about `(R + 0.3 − m)/s` ≈ 2.5–2.8 wraps per level along the
    boundary, about 1000–1250 mm per level, plus about 1000 mm of
    part-air wraps at Z 17 (stock edge).
  - Net **2600 to 2700 s** (−0.3 % to +3.5 %); low confidence.
  - The over-the-bound share falls by about 4 points, and the plan keeps
    the 2D rule everywhere it cuts.
- **C12 heavy test.**
  - The arm modes have no sweep, so the perimeter band count of the
    research modes falls to about the band's body samples. Those samples
    are then judged under the 2D rule.
  - The test should keep the band exemption for the AgentSearch control
    only. That is one change to its `in_perimeter_band` use.
  - Prediction: 0 over still; judged +10 to 20 %.
- **Byte parity.** The grow, the closing guard, the skipped sweep and the
  skim are all behind `is_research_arm` (or `WithinPassLoad`). The shipped
  `planner_outlines`, sweep and `waterline_cleanup` keep their code path.
- **Coverage (`triage.coverage`).** Part 1 cuts at least as far out as the
  sweep did (to `m` from the region, against the sweep's edge at `m`), so
  it leaves nothing new. Part 2 leaves the skipped-region wall stock that
  the waterline cut before. The next op takes it on Corne. The verifier
  must read the hard bar.

### 11.5 What I would change (on the lead's go)

- `adaptive3d/clearing.rs`:
  - `planner_outlines` gets a research branch (the grow and the closing
    guard).
  - `clear_one_region` skips the sweep when the research grow applies to
    the region.
- `adaptive3d/path.rs` and `clearing.rs`: `waterline_cleanup` gets the
  research skim, with a per-level count line.
- The C12 test: the band exemption for the control only.
- No change in `research_geodesic.rs`.
