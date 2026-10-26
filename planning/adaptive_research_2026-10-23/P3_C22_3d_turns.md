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

### 11.6 C31 part 1: built (2026-10-25)

Base f32401326. No cargo, no commit; rustfmt is green. Part 2 (the held
waterline skim) is not built: it waits for the Corne measurement of part 1,
as the lead ruled. Its design stays in 11.3 (4.).

**`adaptive3d/clearing.rs`:**
- `PERIMETER_INSET_MARGIN_MM` (0.25) moves from a local of the sweep to
  the module, with its doc. It is the same value; the forecast's own local
  copy is not touched.
- New `research_grown_outlines(ctx, region, cell, (z, region))`
  (cfg research):
  - It returns `None` unless `is_research_arm`.
  - It grows the region by `R − m` (`OffsetRingSet::from_polygon(..)
    .offset(-(R − m))`, untoleranced flattening: the grow
    `inset_piece_outlines` uses).
  - The closing guard: it keeps the grown outline only when the area of
    the grown outline inset by R is at most the area of the region inset
    by `m`, plus one planner cell times the region's boundary length
    (exterior and holes). The cell is the planner stock's
    `z_grid.cell_size`.
  - It prints one `info` line per decision ("C31 region opening": z,
    region, the three areas, the excess, the allowance, and the decision
    "grown, no sweep" or "closing guard: shipped sweep").
- `clear_one_region`:
  - It computes the decision before the sweep. With the grown outline,
    the sweep's ring list is empty, so no sweep is cut.
  - The outlines are `grown_planner_outlines(grown, R)` (each grown
    polygon split by `inset_piece_outlines`, no smoothing), else the
    shipped `planner_outlines`.
  - The forecast (F-038) still runs first. A skipped region gets no guard
    line.
- `plan_and_emit_region`, `RegionScratch::run` and `pick_region_plan` now
  take the outline list in place of the region polygon. The polygon was
  used only to make that list, so the shipped path makes the same list
  from the same call.

**`tests/adaptive3d_geodesic_arm_c12.rs`:** `read(.., band_exempt)`. The
band exemption is `true` for the AgentSearch control only, so the four
geodesic modes are judged in the perimeter band (stricter).

**Byte parity.** Without `research` `grown` is `None`. With `research` it
is `None` for every shipped arm (`is_research_arm`). The sweep and
`planner_outlines` are then called as before. The constant moved
unchanged.

**Predictions (P):**
- **Guard decisions on Corne "3D Rough 1"** (`geodesic_local_zigzag_pick`):
  4 lines, one per level for the planned region; region 1 of Z 11 / 5 /
  0.5 is skipped by F-038 before the guard. **0 fallbacks.** Corne's walls
  and islands give tool-centre gaps whose rounding radius is R (the
  dilated model), and a closing at `R − m` < R does not fill them. The
  staircase notches of the marching squares add about cell/4 × length,
  under the allowance of cell × length.
- C30's measure: perimeter 1026 → 0 mm; total about 215 mm over 0.471.
  Plunges 55–61; cycle 2600–2700 s (11.4).
- C12 heavy test: 0 over in all four geodesic modes, now judged in the
  band. C12 plain test: unchanged verdicts. The turn moves stay at the
  level, and the larger 2D domain can plan more turns.
- Limit: a sharp reflex corner of the region (two forbidden discs that
  meet) is filled by the closing over a small area. A tool centre planned
  there stands outside the region, and the lift drapes it up over the
  terrain (no gouge; some air). The guard admits it when its area is
  under the allowance.

### 11.7 D19 RED: tool centres of the grown plan read the rim (2026-10-25)

D19 step 3 (`d19/c12_plain.log`), plain test, `geodesic_local_oneway`:
- 11 turn moves are off the level: Z 10.5 (the rim or the island top plus
  the leave), 5.956, 6.279, and 4.82.
- Against D18: turns 42 → 36, entries 68 → 119, moves 2123 → 3015.

**The cause: the §11 premise holds only to one grid cell, and the margin
m = 0.25 mm is smaller than that.**

1. **The region is the tool-centre limit at the walls, but only to a
   cell.** `cell_has_material` reads the drop-cutter heightmap at cell
   centres (the planner grid, cell 0.5 mm; the heightmap and the stock share
   it). Marching squares puts the boundary at the midpoints between an "in"
   and an "out" centre, so the polygon boundary can lie up to cell/2 =
   0.25 mm past the true limit. On a 45° staircase edge it lies 0.18 mm from
   the out centre.
2. **The lift reads the nearest cell.** `lift_to_level` →
   `z_or_bbox_floor_at_world` → `world_cell` rounds to the nearest centre.
   An out cell owns the square of half-width cell/2 round its centre. So a
   planned centre within about 0.5 mm of the polygon boundary on the out
   side's axis reads that cell. Here that cell's floor is the rim
   (Z 10 + 0.5), and the point is lifted to 10.5. The partial values (5.956,
   6.279) are the exact drop-cutter of the emitter's drape. There the centre
   is inside the true forbidden band of the island's sloped facets.
3. **The closing adds to it.** Growing by R − m and insetting by R fills
   every reflex vertex of the marching-squares staircase. At a 45° turn the
   fill is ρ(sec 22.5° − 1) = 2.75 × 0.082 = 0.23 mm. That pushes the 2D
   tool-centre region back toward the out cells by about the margin. The
   guard admits it, because the staircase fills are under cell × length.
4. **The 2D plan steps to its region boundary by design.** The wall pass,
   the arc ends and the turns stand there. So the outer pass of the grown
   plan runs along the out cells and reads the rim wherever the staircase
   or a tie of `round()` puts it in an out square.

**Why the entries nearly double.** Each such point lifts a cut from Z 4.5
to Z 10.5 and back. The lift's large-Z split (`dz` > max(1.1 · Depth/Pass,
1) or slope > `PLUNGE_SLOPE_LIMIT`) puts an entry at each jump, and the
air demotion and the plunge add more. About 25 excursions explain the +51
entries and the +900 moves. The turn count falls (42 → 36) because the
larger 2D domain changes the field's levels (more wraps along the band,
other arc ends).

**The premise, plainly.** It is right in kind and wrong in size. The
region boundary is the tool-centre limit at a wall only to the grid's
half cell, and the nearest-cell lift adds another half cell. A margin of
`PERIMETER_INSET_MARGIN_MM` (0.25 mm, chosen for the boundary clip, O5b)
does not cover one planner cell (0.5 mm). The waterline-load claim of 11.3
(the waterline meets m + one cell) stands; the gouge-free claim needs the
larger margin.

**The proposed fix (not built): grow the shipped smoothed outline, not the
raw region.**
- In the research branch, first smooth the region exactly as
  `planner_outlines` does (`SIMPLIFY_INSET` 0.3 inward, then RDP
  `SIMPLIFY_TOLERANCE` 0.3; they move to module scope unchanged). Then grow
  each smoothed polygon by R − m and split it into pieces.
- The 2D tool-centre region is then the smoothed polygon inset by m: 0.55
  to 0.85 mm inside the raw region boundary.
  - That is beyond the out cells' nearest-read squares (at most 0.5 mm
    from the boundary on an axis edge, about 0.43 mm on a diagonal).
  - It is beyond the true limit by at least 0.30 mm, since the boundary is
    within cell/2 of the true limit.
  - The RDP removes the staircase, so the closing fills almost nothing
    (the guard's excess falls to about 0).
- No new constant: R, `m` and the two shipped smoothing values.
- The waterline then meets (0.55…0.85) ± cell/2, which is 0.3 to 1.1 mm,
  or at most 0.18 D. That is under c = 0.219 at s 1.2 and under 0.3626 at
  s 2. The sweep is still not needed: the smoothed-and-grown 2D domain
  reaches 0.3 to 0.6 mm further out than the sweep's centre locus did (the
  sweep's centres were at R + 0.25 inside the raw region; the 2D centres
  are now at 0.55 to 0.85 inside it).
- An exact alternative is a post-check: drop (do not plan) any 2D point
  whose nearest heightmap cell is out of the region. That needs the 2D
  planner to read the 3D heightmap (`adaptive/`, not my files), so I do not
  propose it.

**Predictions with the fix (P):**
- Plain C12 test: every turn move at Z 4.5. Entries about 68 to 80 (D18
  68, plus the wraps of the 2–3 mm band along the walls). Turns about the
  D18 count (±6).
- Heavy C12 test: 0 over, judged in the band.
- Corne: the guard keeps all 4 planned regions. The C30 perimeter over
  length goes to 0, and the C31 cycle and plunge predictions of 11.4
  stand.

### 11.8 C31 part 1, the D19 fix: built (2026-10-25)

No cargo, no commit; rustfmt is green. All changes are in
`adaptive3d/clearing.rs`.

- **The shipped smoothing, moved verbatim.**
  - `SIMPLIFY_INSET` and `SIMPLIFY_TOLERANCE` (0.3, 0.3) move to module
    scope with docs.
  - The smoothing block of `planner_outlines` becomes
    `fn smoothed_region(region, params_2d)`: the same text; a script diff
    against HEAD is identical. `planner_outlines` calls it.
- **`research_grown_outlines(ctx, region, params_2d, cell, (z, region))`**
  now:
  1. smooths the region with `smoothed_region`;
  2. grows each smoothed polygon by `R − m` (`OffsetRingSet`,
     untoleranced);
  3. gives the grown polygons to `grown_planner_outlines` for the piece
     split.
- **The region keeps the shipped path** (smoothing and sweep) when the
  first of these is true. The `info` line "C31 region opening" names it in
  `decision`, beside `cell_mm`, `nominal_margin_mm`, `measured_margin_mm`,
  the three areas, `excess_mm2` and `allowed_mm2`:
  1. "no smoothed or grown outline".
  2. "inside margin under one planner cell": `SIMPLIFY_INSET + m < cell`.
     This is the lead's condition: the reach of an out cell's nearest read
     plus the boundary error, about one cell.
  3. "measured margin under one planner cell" (lead's correction; first
     built as half a cell): the least distance between the smoothed and
     the raw boundary, plus `m`, is under one cell.
     The RDP can move the smoothed outline back out by up to its tolerance,
     so the nominal inset is not a bound. Half a cell is the boundary error
     of marching squares (the midpoint between an in and an out centre).
     The nearest-read reach inside the raw polygon is at most cell/(2√2) on
     a 45° edge, and 0 on an axis edge, so half a cell covers both. The
     separation is exact for two boundaries that do not cross: they come
     nearest at a vertex of one of them (`boundary_separation`).
  4. "closing guard": the excess over one planner cell times the smoothed
     boundary length.
  5. Else "grown, no sweep".

**The cells.** The planner cell is `max(R/6, tolerance)`, and the planner
stock's `z_grid.cell_size` is that value.
- Corne "3D Rough 1": R 3, tolerance 0.1, so the cell is **0.5 mm**.
- C12 fixture: R 3, tolerance 0.1, so the cell is **0.5 mm**.
- Condition 2 then reads 0.3 + 0.25 = 0.55 ≥ 0.5: **0.05 mm of room** on
  both. A tolerance above 0.55 mm, or a tool radius above 3.3 mm at
  tolerance ≤ R/6, makes it fail. Such a region falls back, by design.
- Condition 3 at cell 0.5 reads `measured + 0.25 ≥ 0.25`, which always
  holds (a distance is never negative). It binds only when cell/2 > m,
  that is, a cell over 0.5 mm. The run prints `measured_margin_mm` per
  region all the same.

**Predictions (P).**
- Corne: 4 decision lines, all "grown, no sweep". `measured_margin_mm`
  about 0.35–0.55 (the RDP moves a staircase outline back out by up to its
  half amplitude, about 0.1–0.2 mm).
- C12 plain test: all turn moves at Z 4.5; entries about 68–80; turns
  about D18's 42 ± 6.
- Heavy test: 0 over in the band.
- Byte parity: green (the smoothing moved verbatim; the research branch is
  gated).

**The D19 heavy-test red (`phase2/d19_c12_test_output.txt`, at the raw-grow build; explanation only, no code).**

`geodesic_curv` reads 5 judged samples over the exact bound:
- move 496 at (12.81, 15.62, 7.00) and (12.69, 15.88, 7.00): exact 0.999;
- move 511 at (16.75, 13.21, 7.00): exact 0.755;
- move 2126 at (28.77, 5.12, 4.50) and (28.64, 5.38, 4.50): exact 0.999.

Against D18 (2298 moves, entry samples 1410, judged levels {4.5, 7.0, 7.5,
8.0, 10.5}), D19 has 3081 moves, entry samples 2265, and judged samples at
**Z 5.0, 7.1, 7.2, 7.3, 7.4, 7.8, 7.9 and 8.9**. Those Z values do not
belong to any level. They are cuts that the raw-grow plan lifted off its
level: the out-cell reads of 11.7, at the rim (10.5) and partial
(drape) heights. The extra entry samples (+855) are the lift's large-Z
splits round them.

**What move 496 is (inferred; D19 kept no toolpath).** It is a level cut
that meets stock an earlier pass of the 2D plan was meant to remove, but
the lift raised.
- The 2D replay held each pass to the ceiling on the 2D grid, where the
  earlier passes had cut at the level.
- In 3D a stretch of an earlier pass read an out cell or a raised drape
  floor. The lift raised it (to 7.1–8.9, or 10.5). A short air run is
  re-promoted and cut at the raised Z; a long one becomes a rapid with the
  large-Z split round it.
- The planner stamped what was emitted (G-PLANSIMGAP): the raised pass.
  So the stock between the level and the raised Z stayed under that
  stretch.
- The next pass of the 2D plan that crosses it meets that stock across
  its whole disc: full width (the exact reading's 0.999).
- It may be only as thick as the lift (0.1–1.9 mm), because the exact
  oracle counts any layer left between two heights as full-width stock
  (C30.6). It is still a real 2D-rule breach: the 2D rule is radial.

How the positions fit:
- Move 2126 is 5.23 mm from the x = 34 wall. That is one stepover (2 mm)
  inside the 2D boundary of the raw grow, which is about R + m = 3.25 mm
  from the wall. So it is the pass after a lifted boundary stretch.
- Move 496 (8.4 mm from the y = 24 wall, 15 mm from the island) is
  further in. A wrap ends where the level set meets the 2D boundary, so a
  lifted arc end leaves a stub of standing stock. The next wrap or a link
  that cuts through the stub can stand some wraps inward of the boundary.
  So "a cut that resumes after a rim lift and its split" fits, in the wide
  sense: the standing stock comes from a lifted earlier pass.
- I cannot name the exact earlier move without the toolpath.

**Does the smoothed-grow fix remove it? Yes, if the cause is as above.**
- With the fix, the 2D tool-centre region is at least 0.55 mm (nominal)
  inside the raw region boundary. At cell 0.5 that is beyond any out
  cell's nearest-read reach and beyond the true limit (11.7–11.8). The RDP
  has removed the staircase, so the closing fills almost nothing.
- So no planned point reads an out cell, the lift raises nothing, and no
  stretch of the 2D plan is left unstamped.
- What remains are the drape raises of the flat floor, which D18 also
  had: its judged levels 7.5, 8.0 and 10.5, with 0 over.

**Prediction for D19b (the smoothed-grow build; P):**
- `geodesic_curv` and the other three geodesic modes: **0 over the exact
  bound**.
- Judged levels back to D18's set ({4.5, 7.0, 7.5, 8.0, 10.5}, perhaps a
  stray drape level). No Z 7.1–7.4, 7.8, 7.9 or 8.9.
- Entry samples close to D18's 1410 (within +10 %). Moves 2298 + the band
  wraps: about 2400–2700.
- Perimeter band 0 (no sweep). Waterline at most a few samples over the
  screen (D19: 0; D18: 27).
- If a sample is still over, the first check is whether its Z is a level
  value and whether an earlier move at the same XY ran above the level. A
  probe that prints the moves within R of the sample, with their Z, would
  show it.

**Correction (lead, before D19b): condition 3 now uses one planner cell.**
- The threshold is `measured + m ≥ cell`, as in condition 2. At half a
  cell it could never trip at cell 0.5 (`measured + 0.25 ≥ 0.25`), even
  with a measured separation of 0: exactly the D19 case.
- The one-cell reach is the boundary error (up to cell/2) plus the
  nearest-read square (up to cell/2), as 11.7 diagnoses.
- My earlier text above, which said half a cell covers both, is withdrawn.
- The code is `measured_margin < cell`, and the `info` line says "measured
  margin under one planner cell: shipped sweep".

**Prediction with the one-cell condition (P, replaces the guard line of
11.8's predictions):**
- A region keeps the grow only if the smoothed outline stays at least
  0.25 mm inside the raw boundary everywhere (0.25 + 0.25 ≥ 0.5).
  - On axis and 45° wall stretches the smoothed outline is the 0.3 mm
    inset, so they pass.
  - Where the RDP replaces a staircase of another angle, or a staircase
    circle (the C12 island, r 5 on the 0.5 mm grid; the Corne hex holes),
    the chord can come back out by up to about half the staircase
    amplitude. Then `measured` can be about 0.1–0.2 mm.
- So it is likely (about 60 %) that the C12 region at both levels, and some
  Corne regions, **fall back** to the smoothing and the sweep.
- If the C12 region falls back, the heavy test now judges the geodesic
  arms in the sweep band (the band exemption is the control's only), so
  it reads the sweep slot over the bound and **fails** in every geodesic
  mode. That would be the honest result: the grow did not apply there. The
  `measured_margin_mm` in the D19b log tells which case it is.
- If it falls back, the plain test is unchanged from D18: turn moves at
  the level, entries about 68.
- If the C12 region keeps the grow: 0 over, as predicted above.

**The shortfall inset (lead, before D19b): built.** In
`research_grown_outlines`:
- After the smoothing, `measured_before` is the least distance between the
  smoothed and the raw boundaries (`boundary_separation`), plus `m`.
- If it is under one cell, every smoothed polygon is inset by the
  shortfall `cell − measured_before`, once, region-wide (`offset_polygon`),
  and the separation is measured again (`measured_after`).
- The grow, the closing guard's areas and the boundary length are then
  taken on the inset outline.
- The decision order:
  1. no smoothed or grown outline;
  2. "the shortfall inset split the outline" (more polygons after the
     inset than before);
  3. the nominal margin under one cell;
  4. "measured margin under one planner cell" on `measured_after`;
  5. the closing guard;
  6. else "grown, no sweep".
- The `info` line prints `measured_before_mm`, `shortfall_inset_mm` and
  `measured_after_mm`.
- Why the inset is enough: the smoothed region lies inside the raw one
  with boundary separation `d`. A point of its inset by `k` has a disc of
  `k` inside the smoothed region, hence a disc of `k + d` inside the raw
  region. So `measured_after ≥ measured_before + k = cell`, up to the
  offset's flattening, which only cuts inward. The check confirms it.
- No new constant.

**Prediction (P), the lead's, which I share:**
- Every region of C12 (both levels) and of Corne (4 planned) keeps the
  grow: decision "grown, no sweep", `measured_after_mm` ≥ 0.5.
- `shortfall_inset_mm` is 0 where the outline stays 0.25 inside, and about
  0.05–0.15 on the C12 island and the Corne hex holes. No split is
  expected: the necks are much wider than twice the shortfall.
- So the D19b predictions of 11.8 stand:
  - heavy test 0 over in all four geodesic modes, judged in the band;
  - plain test turn moves at the level, entries about 68–80;
  - judged levels back to D18's set.

### 11.9 D19b: every C12 region fell back; which condition, and the record (2026-10-25)

D19b: the plain test equals D18 exactly, and the heavy test fails
`geodesic_curv` with 632 samples over (exact 1.000, move 6, wall distance
6.00: the old sweep band). So every C12 region kept the shipped sweep. My
prediction that every region keeps the grow was wrong. The `info` lines
do not print in a test binary (no tracing subscriber).

**(2) Which condition refuses (read from the code and the fixture; best
estimate).**

| Condition | C12 reading | Trips? |
|---|---|---|
| 1. no smoothed or grown outline | the pocket region smooths and grows | no |
| 2. shortfall inset split | an inset of 0.15–0.25 mm cuts no neck (the narrowest part of the region is the pocket band round the island, about 13 mm) | no |
| 3. nominal margin | 0.3 + 0.25 = 0.55 ≥ 0.5 | no |
| **4. measured margin after the inset** | **about 0.490–0.4999 against 0.5** | **yes** |
| 5. closing guard | the pocket's corners are convex from the region; the island hole is a disc of radius about 8.3 > R − m = 2.75. Excess about 0 against an allowance of about 0.5 × 260 = 130 mm² | no |

Why condition 4 trips, by construction:
- **Before the inset.** The pocket walls are axis edges, so the smoothed
  outline stands 0.3 mm inside there. The island hole is a marching-squares
  staircase circle. Its 0.3 mm grow, then the RDP at 0.3 mm, gives chords
  whose midpoints come back toward the island by up to their sagitta, and
  the raw staircase notches stand out toward them. So `measured_before` is
  about 0.25–0.35 (separation 0.0–0.1, plus m).
- **The inset.** The shortfall is 0.15–0.25. After the inset, the separation
  is at least the old separation plus the inset (11.8): exactly
  `cell − m`, so `measured_after` is exactly the cell, in theory.
- **The flattening.** `offset_polygon` flattens each arc join "on the
  input side of the true offset", up to `FlattenPolicy::UNTOLERANCED_MM`
  (0.010 mm; `polygon.rs` P6.0). For an inset, the input side is outward,
  toward the raw boundary. So the inset outline stands up to 10 µm less
  inside than asked, and `measured_after` lands a few µm under 0.5. The
  strict test `measured_after < cell` then refuses.
- My 11.8 note said the flattening "only cuts inward". That is wrong for
  an inset, and it is withdrawn.

**The closing guard is not comparing the wrong pair.** Since the shortfall
inset, both of its areas come from the same inset outline S′: the grown
outline (S′ grown by R − m) inset by R, against S′ inset by m. So it
measures the closing of S′ only, which is what it is for. It never compares
against the raw region inset by m. Its allowance (cell × S′'s boundary
length) is large here, so it is not the refusal.

**The fix (built with the record, below).** The shortfall inset adds the
flattening budget: `shortfall = cell − measured_before +
FlattenPolicy::UNTOLERANCED_MM` (an existing constant). Then
`measured_after ≥ cell` holds with the flattening error inside it. The
condition stays strict.

**(1) The decision is now in the research record the tests read.**
- `adaptive3d/mod.rs`, cfg research:
  - new `pub struct RegionOpening` (z_level, region, cell_mm,
    nominal_margin_mm, measured_before_mm, shortfall_inset_mm,
    measured_after_mm, excess_mm2, allowed_mm2, `decision: &'static
    str`);
  - new `pub struct Research3d { turns, openings }`;
  - `adaptive_3d_research_output_with_cancel` now returns
    `(Adaptive3dOutput, Research3d)`, and `research_turn_feeds` fills both.
- `adaptive3d/clearing.rs`: `ClearZLevelContext::research_openings`
  (RefCell, cfg research). `research_grown_outlines` pushes one record per
  decision; the `info` line stays.
- `adaptive3d/path.rs`: the context initialiser and
  `Adaptive3dSegmentsResult::research_openings` (the hand-over).
- `tests/adaptive3d_geodesic_arm_c12.rs`:
  - `plan` returns `(out, turns, openings)`.
  - Both tests print one line per opening: decision, cell, nominal,
    measured before, the shortfall inset, measured after, and the closing
    excess and allowance.
  - The heavy test asserts, per geodesic mode and before the load
    assertion: at least one opening, and every opening "grown, no sweep"
    or "no smoothed or grown outline". A region too thin to smooth also
    has an empty sweep ring. The assertion prints the refused lines.

**Prediction for D19c (P):**
- The C12 lines read: "grown, no sweep" at both levels, measured before
  about 0.25–0.35, shortfall about 0.16–0.26, measured after 0.500–0.510.
- Then the D19b predictions of 11.8 apply: 0 over; judged levels close to
  D18's set; entries about 68–80 in the plain test.
- If the record shows another condition, it names it with the numbers.

### 11.10 D19c: 4 moves over in the heavy test (`geodesic_curv`); diagnosis (2026-10-26)

D19c (`phase2/d19c_c12_test_output.txt`): every opening is "grown, no
sweep" (measured after 0.5100; the flattening budget works). The heavy
test fails `geodesic_curv` with 4 judged moves over both bounds:

| Move | At | Distance to | Sim / exact |
|---|---|---|---|
| 435 | (-2.50, -8.59, 7.00) | island edge 3.95 (r 8.95 from the island centre) | 0.685 / 0.707 |
| 533 | (12.93, 5.52, 7.00) | r 14.06; y wall 18.5 | 0.815 / 0.957 |
| 1711 | (-2.57, -8.88, 4.50) | island edge 4.24 (r 9.24) | 0.667 / 0.775 |
| 1805 | (17.82, 12.12, 4.50) | r 21.6; y wall 11.9 | 0.833 / 0.975 |

The test kept no toolpath, so the producer of each move is not read from
data yet. What the output does show:

- **Q3: the pairs.** 1711 − 435 = 1276 and 1805 − 533 = 1272: the Z 4.5
  plan repeats the Z 7 plan move for move, about 1274 moves later. 435 and
  1711 stand at the same XY, so they are the same pass of the 2D plan at
  both levels. 533 and 1805 are about the same move of the plan (533 +
  1274 ≈ 1807), on a long pass whose over sample sits at other XY on the
  two levels. So there are **two causes, each repeated per level**, and
  both lie in the plan and its 3D emission. They are not stock carried from
  the level above.
- **A new judged level.** D19c judges samples at Z 10.0
  (`{45, 70, 75, 80, 100, 105}`); D18 had none (`{45, 70, 75, 80, 105}`).
  Z 10.0 is a draped floor: a drop-cutter height of 9.5 plus the 0.5
  leave. Only a centre whose disc lies on a sloped facet near the top of
  the island (or of a pocket wall) reads it. So **the grown 2D domain now
  puts tool centres in the drape ring of the walls.**
- **Q2: did the shipped sweep cut this stock before?**
  - 435 / 1711 (r 8.95–9.24) lie in the old island sweep's swath: its
    centre ring at r ≈ 8.0 + 3.25 = 11.3 cut r 8.3–14.3. Yes, the sweep
    cut it before.
  - 533 (r 14.06) is at the outer edge of that swath. Before, the sweep
    cut its inner part; the 2D plan's passes started outside.
  - 1805 (r 21.6, 11.9 from the y wall) lies outside every old sweep swath.

**H1, the island pair (435 / 1711): a 2D pass in the drape ring, lifted.
Medium-high confidence.**
- The premise of 11.3 is wrong a second time, in a way 11.7 missed. The
  region polygon is not the tool-centre limit at the level. It is where
  stock stands above `max(drop-cutter + leave, z_level)`
  (`build_material_bool_grid`), so its boundary is where that floor
  reaches the stock top (here Z 10). At a sloped facet (the island's edge
  slopes over one mesh step, r 5.0–5.7; the pocket walls over 34.0–34.5)
  the region holds a **drape ring** of centres whose floor lies between the
  level and the stock top: the island ring is r ≈ 8.0–8.7.
- The grown 2D domain starts 0.51 mm inside the region boundary (r ≈
  8.56), so it ends inside that ring. The old domain started at r ≥ 11 and
  never did.
- A 2D pass in the ring is lifted (the drape) to its contact Z. Its disc's
  outer part then stands above the level over flat floor, so it leaves a
  layer of level stock that the 2D grid holds cut. The next pass outward
  (r ≈ 8.6 + s ≈ 10.6; the oracle's reading at r 8.95–9.24 is on that
  pass's disc) meets that layer across its disc: exact 0.71–0.78. The
  oracle counts any layer as full width (C30.6), and the 2D rule is radial,
  so it is a real breach.

**H2, the long pass (533 / 1805, 0.96–0.98): not explained yet. Low
confidence.**
- A near-full slot far from the walls means a pass met stock on both
  sides of its disc, that is, stock that no earlier move cut.
- Candidates:
  - (a) the same H1 layer reached by a later pass that crosses the ring
    band (a link or a wrap end that turns back across it);
  - (b) the meeting line behind the island, where the two fronts of
    `geodesic_curv` (C5, meeting "split") meet. The larger domain moves
    the seed and the meeting line, and a cascade there is a known
    full-width producer (C5).
- I cannot tell (a) from (b) without the moves.

**The probe (built; test code only, no production change):**
`tests/adaptive3d_geodesic_arm_c12.rs`.
- Each move over the exact bound gets a `probe`:
  - the 12 moves before it and the 2 after it, with intent, kind (rapid
    or feed), end point and XY length;
  - every earlier feed move whose XY segment passes within R of the
    sample, counted as at the level, above it, or across or below it,
    with the last eight listed.
  This names the pass (wrap, seam, link: `Linking`, lobe start after an
  entry) and shows whether the passes that should have cut the stock ran
  lifted (H1: "above it").
- The heavy test now runs every mode and reports all failures together at
  the end (openings, the two levels, over, turn faults, turns planned),
  each with its probes. The other four geodesic modes therefore give
  their numbers too.

**The proposed fix, for H1 (not built; it needs the probe to confirm H1):
grow from the level's flat floor, not from the material region.**
- In `research_grown_outlines`, the base is the region's cells whose floor
  is the level: `surface + leave ≤ z_level`, the cells that do not drape.
  It is built as the region is (bool grid, marching squares,
  containment), then intersected with the region. Then come the smoothing,
  the shortfall inset (cell − m against this base) and the grow by R − m.
- The 2D centres then stand at least one cell inside the flat floor, so no
  planned centre drapes. The drape ring stays the waterline's, as it was
  before C31.
- Guard: when the flat base is smaller than the region by more than one
  cell times the region's boundary length (terrain: most of a level
  drapes), the region keeps the shipped path.
- No new constant: the cell, R, `m`, and the two smoothing values. On the
  C12 pocket the base differs from the region only by the two drape rings
  (about 0.7 × 260 ≈ 180 mm² against an allowance of about 130 mm²).
  **So the guard as stated would refuse C12.** The allowance must instead
  read the ring against the slope's own width. That is not settled. I want
  the probe run before choosing it.

**Request:** run the heavy test once (D19d) with the probe. It costs no
production change. It answers Q1 for all 4 moves and H2's (a) against
(b). I will then fix by evidence.

### 11.11 The H1 guard on paper: the drape-ring allowance (2026-10-26; no code until D19d)

The lead's stop rule: the build after D19d is the last C31 round before G2.
If it is not green on the heavy test, C31 part 1 is dropped and G2 runs
with the shipped perimeter sweep. The fix must be small and inside the
research arm.

**What the guard must decide.** With the H1 fix, the 2D plan is grown
from the level's flat floor F (the region cells whose floor is the level:
surface + leave ≤ z_level). The drape ring D = region − F (centres whose
floor lies between the level and the stock top) is then left to the
waterline cleanup, as before C31. So the region may take the grown path
only when the waterline can cut that ring under the ceiling. Otherwise
(terrain, where most of a level drapes) it keeps the shipped path.

**The waterline's load at a wall, with the fix.** The waterline centre runs
on the level's tool-centre limit (the mesh contour at z, offset by the
cutter): that is the boundary of F. On its two sides it meets:
- toward the pocket, the band between its path and the 2D plan's outer
  pass. The 2D centres stand at least one cell inside F (the margin
  condition of 11.8), so this band is about one cell wide, or less with the
  quantum;
- toward the wall, the stock over the drape ring, whose width in centre
  space is the ring's width `w` (the slope's horizontal extent, read
  through the grid).
So its swept width is at most `w + cell`.

**The allowance (derived; no new constant).** Hold the waterline to the
2D ceiling:

`w_max + cell ≤ c·D`, that is, `w_max ≤ c·D − cell`,

with `c` the replay's own ceiling (`PassLoad::swept_width(s, R, cell)`),
D = 2R, and the cell the planner cell. `w_max` is measured, not assumed:
it is the largest distance from a vertex of the region boundary to the
boundary of F (the `boundary_separation` reading taken as a maximum
instead of a minimum). The mesh step enters only through what the
measurement reads (a facet that slopes over one mesh step gives a ring of
about that step, √2 steps on a diagonal), so the guard needs no mesh
input.

| Fixture | s, R | c·D | cell | allowance `c·D − cell` | expected `w_max` |
|---|---|---|---|---|---|
| C12 (island and pocket slopes over one 0.5 mm mesh step) | 2, 3 | 2.176 | 0.5 | **1.68 mm** | about 0.7 + up to one cell of quantum: 0.7–1.2 → passes |
| Corne "3D Rough 1" (near-vertical walls) | 1.2, 3 | 1.313 | 0.5 | **0.81 mm** | about the cell quantum, 0.25–0.5 → passes, with a thin margin |
| Terrain (rivmap, wanaka) | — | — | — | — | tens of mm → shipped path, by design |

Notes:
- This replaces the area form of 11.10 (`cell × length`), which compared
  the ring area with a quantum allowance and would have refused C12
  (about 180 mm² against 130). The width form compares the ring with the
  load the one tool that cuts it may carry.
- It also replaces the waterline-load claim of 11.3. With F as the base,
  the waterline meets `w + cell`, not `m + cell`, and the guard holds
  exactly that to the ceiling.
- The existing conditions stay: the margin (now on F), the shortfall inset,
  the split, and the closing guard (on the smoothed F).

**The build after D19d, if the probe confirms H1** (the passes that should
have cut the over stock read "above it", that is, lifted):
1. `research_grown_outlines`: build F as the region is built (the bool
   grid with the extra test `surface + leave ≤ z_level`, marching squares,
   containment), intersected with the region polygon. Then smooth, inset
   and grow from F.
2. The new condition "drape ring wider than c·D − cell: shipped sweep",
   recorded with `w_max_mm` in `RegionOpening`.
3. Nothing else changes.

If the probe instead shows H2 (b), the meeting line, the H1 fix does not
cover moves 533 and 1805. Then, by the stop rule, I report that C31 part 1
cannot be made green in one round.

### 11.12 D19d: H1 not confirmed; the arc-end hook; the assertion scoped (2026-10-26)

D19d (`phase2/d19d_c12_test_output.txt`, the heavy C12 test with the
probe), read by the lead:
- **The G1 arms are green.** `geodesic_local_oneway`,
  `geodesic_local_zigzag` and `geodesic_local_zigzag_v2` read 0 over the
  exact bound and perimeter band 0, and every region opens "grown, no
  sweep". Their turns over the screen are the known, fed turns.
- **Only the far-side modes fail:** `geodesic_curv` 4 over (peak 0.975),
  and `geodesic_curv_split` 2 over (the 435 / 1711 pair, its moves 404 and
  1587).
- **H1 is not confirmed.** Probe 435: 3 earlier feed moves within R at the
  level, 0 above it. No lifted pass stood there.
- **Each over move is the last segment of an arc.** It is a short sideways
  hop (1.0–2.0 mm) off the arc's line into stock, and a lift follows at
  once:
  - 435: (-3.31, -8.38) to (-2.34, -8.63);
  - 533: (12.04, 6.17) to (13.11, 5.40);
  - 1805: (15.99, 12.12) to (18.02, 12.12).
  That is an arc-end hook of the plain-seam arm (no turns): the seam step
  of `geodesic_curv` that leaves the arc toward the next wrap's start,
  which the 3D emission cuts and then lifts out of. Before C31 the
  perimeter sweep had removed that stock (435 / 1711 lie in its old
  swath), so the hook read under the bound.

Decision (the lead; stop rule, smallest change):
1. The H1 flat-floor fix is **not built**. §11.11 stays as a parked design.
2. `tests/adaptive3d_geodesic_arm_c12.rs`:
   - The heavy test asserts the exact bound on the G1 arms only: the three
     arc-link modes and `geodesic_local_zigzag_pick`, which joins the heavy
     modes.
   - `geodesic_curv` and `geodesic_curv_split` still run and print their
     over samples and probes, as attribution, with no assertion on their
     over count. Their other checks stay.
   - The module doc says so, with the reason and the defect tag; the
     `heavy:` reason now names seven plans (about 16 min in debug).
3. `DEFECTS.md`: G-ARCENDHOOK (parked; research arms only, not shipped).
4. No other change. C31 part 1 stays as built (11.6–11.9). The probe
   stays in the test.

Prediction for D19e (P):
- The heavy test is green.
- The G1 arms read as in D19d; `geodesic_local_zigzag_pick` reads like
  `geodesic_local_zigzag` (the C29 pick keeps A on the C12 pocket, and in
  D19c the plain test's pick equalled zigzag).
- The two far-side modes print 4 and 2 attribution samples, as in D19d.

### 11.13 C34: wanaka200 Z 4.0 region 1, a grown centre area 30 times the region (2026-10-26)

D20 (`phase3/fix7/wanaka200/geodesic_local_zigzag_pick/stdout.txt`, at
b37cad1a4), the "C31 region opening" lines:

| z | region | region area | centre area | target area | measured before / after | decision |
|---|---|---|---|---|---|---|
| 19.54, 14.08, 8.62 | 1 | 40099.9 | 39631.1 | 39631.1 | 0.550 / 0.550 | grown, no sweep |
| 4.0 | 1 | **2580.5** | **77602.8** | 1157.2 | 0.250 / 0.509 | closing guard: shipped sweep |
| 4.0 | 3 | 159.9 | 108.1 | 107.0 | 0.253 / 0.510 | the shortfall inset split the outline: shipped sweep |

**What region 1 is.** At the three upper levels the region is the full
square of the model's footprint (40099.9 mm², about 200.25 mm a side). At
Z 4.0 it is 2580.5 mm². The fit that matches every number is the same
square with a hole of about 37519 mm² (40099.9 − 2580.5): **a frame ring
about 3.2 mm wide round the footprint**. The terrain stands above Z 4.0 +
leave over the rest, so the level holds only the ring at the edge.

**Why the target is under half the region: expected, not a defect.** The
target is the outline after the smoothing inset (0.3, plus the RDP), the
shortfall inset (0.26) and the margin (0.25): about 0.81 mm off each of the
ring's two sides. Over a centre line of about 790 mm that removes
2 × 0.81 × 790 ≈ 1280 mm², leaving about 1300 mm² (read 1157; the RDP
takes a little more). A 3.2 mm band loses half its area to 0.81 mm a side.

**Why the centre area is 30 times the region: the hole became a filled
polygon in the grow.** The number splits exactly into two parts:
`77602.8 = 39631.1 + 37971.7`.
- 39631.1 is to 0.1 mm² the centre area of the upper levels, that is, the
  **filled** square (no hole) taken through the grow and the inset.
- 37971.7 is the hole as a filled island. The hole of the inset outline
  (about 37519 + 0.81 × 780 ≈ 38150 mm²), grown OUTWARD by R − m
  (+2.75 × 780 ≈ +2150) and then inset by R (−3 × 780 ≈ −2340), gives
  about 37960 mm².

So after the grow, the exterior has no hole and the hole's ring is a
separate positive polygon that the grow offset as a boundary (outward),
not as a hole (which would have shrunk by 2.75). A grow of the wrong sign
would shrink the exterior, so it is ruled out. A plain lost hole would
read about 39 600 mm², so that is ruled out too. What fits is an
**orientation or classification fault of the hole ring in the grow
stage**.

Where it can happen, from the code:
- `research_grown_outlines` grows with
  `OffsetRingSet::from_polygon(s).offset(-(R − m))` (the cascade,
  `RingGroup::offset`, cavalier `Shape`). `Shape::from_plines` classes each
  ring by its own orientation (CCW a boundary, CW a hole). A hole ring that
  reaches it counter-clockwise is offset as an island: outward for a
  negative distance. The output then has it as a second CCW boundary,
  which `to_polygons` returns as its own polygon. Every number fits that.
- The same outline goes through `offset_polygon(s, m)` for the target,
  and the target is correct (1157). Both paths repair a self-intersecting
  polygon first (`repaired`, a geo boolean union), and both build the hole
  polyline the same way. So the fault would have to be in the cascade's
  negative (outward) offset of a polygon with a hole, or in a ring that
  only the cascade's input sees: the outline after the RDP has a hole
  whose winding the RDP can flip, if it drops enough of a thin ring.
- Reading alone does not decide between these. **My guard is not the
  defect.** It read a true excess and refused, as it should: the output is
  safe. But a ring-shaped region can never take the grow while this
  stands.

**Region 3: expected.** A 160 mm² region whose shortfall inset cuts a
neck: one region becomes two, and the split condition sends it to the
shipped sweep, as designed.

**The probe (built; test file only; not run): `c34_frame_ring_grow_probe`**
in `tests/adaptive3d_geodesic_arm_c12.rs`.
- It is `#[ignore = "instrument: …"]`, asserts nothing, and runs in
  milliseconds.
- It builds the same frame ring (a square of 40100 mm² with a CW square
  hole of 37520 mm²) and runs the steps of `research_grown_outlines` less
  the RDP: the 0.3 inset, the 0.26 shortfall inset, the grow by R − m
  (through `OffsetRingSet` and, for comparison, through the single-shot
  `offset_polygon(·, −(R − m))`), the centre (inset by R), and the target.
- It prints each stage's polygons, with the exterior's and each hole's
  signed areas.
- Command (operator PC, a few seconds after the build):
  `scripts/cargo_lane.sh test -p rs_cam_core -q --features research,test-support --test adaptive3d_geodesic_arm_c12 -- --ignored --nocapture c34_frame_ring_grow_probe`

**Reading the probe, and the fix it would pick (research arm only):**
- If the `OffsetRingSet` grow returns the hole as a second positive
  polygon and the single-shot grow returns one polygon with a smaller hole,
  the cascade's outward offset with holes is at fault (a `polygon.rs`
  defect, outside my files; file it). The research fix is to grow with the
  single-shot `offset_polygon(s, −(R − m))`, which flattens its arcs once,
  and that is enough for a shape the planner insets once.
- If both grows are right on the synthetic ring, the fault needs the real
  ring: the RDP of a staircase hole, or a self-intersection repair. The
  next probe is then a research-only dump of the region and the outline
  at each stage, for a ring region whose excess exceeds its allowance.
  That is a production (research-gated) change, so I would ask first.

**C34 probe result (D9, `phase3/c34_probe_output.txt`, rc 0): the second
outcome. The cascade is right on the synthetic ring.**

| Stage | Exterior | Hole | Area |
|---|---|---|---|
| region | 40100.0 | −37520.0 | 2580.0 |
| inset 0.3 (no RDP) | 39860.1 | −37752.7 | 2107.3 |
| shortfall inset 0.26 | 39652.7 | −37954.9 | 1697.8 |
| grown R − m, `OffsetRingSet` | 41866.8 | −35842.4 | 6024.4 |
| grown R − m, `offset_polygon` | 41866.8 | −35842.4 | 6024.4 |
| centre (grown inset R) | 39453.8 | −38142.4 | 1311.4 |
| target (inset m) | 39453.8 | −38149.6 | 1304.2 |

- Both grows keep the hole as a hole, with the same numbers. The centre
  equals the target to 7.2 mm² (against an allowance of about 0.5 × 1580
  ≈ 790 mm²). So on a clean frame ring with a CW hole the whole chain is
  right, and the guard reads it right.
- So G-CASCADEHOLE is **not** filed: the probe does not show a fault of
  the shared cascade in `polygon.rs`.
- The D20 fault needs the real ring. What the synthetic probe leaves out:
  - the marching-squares staircase of both rings;
  - the RDP at 0.3 on a ring that is only 3.2 mm wide (it can bring the
    two rings within 0.3 of each other, or make a ring cross itself, and
    then `repaired()`, the geo union, rebuilds the polygon);
  - the stock corner, where the frame's exterior is the centre boundary.
  One of those turns the hole into a boundary before or during the grow.
  The target path, on the same outline, stays right, so the trigger is
  in what the outward offset does with that ring.
- Next step (parked until after G2, lead 2026-10-26): a research-gated
  dump of the region and of the outline at each stage, for every region
  whose closing guard refuses (excess over its own allowance), or gated by
  an env flag. No "factor of ten": that would be a new constant. That is one JSON
  file per such region under the run's output, through the existing
  `RegionOpening` record or the debug trace. It is a production
  (research-gated) change. Until then, ring regions fall back to the
  shipped sweep; the output is safe.

Parked as G-RINGGROW in `DEFECTS.md` (lead, 2026-10-26).

## 12. C35 design: one spiral while the wraps are closed (R4 item 2) (2026-10-26)

DESIGN; no code yet, no cargo. Inputs: `R4_independent_review.md` §1.2,
§2 and §5 (item 2); `phase3/fix9/README.md` §4; the worktree at b37cad1a4
(plus the uncommitted C12 test file).

### 12.1 What the code does now (read)

- The zigzag arms (`C20_LOCAL_ZIGZAG`, `C23_ZIGZAG_V2`, `C28_ZIGZAG_PICK`)
  run `JoinRule::Seam`. In `plan_piece` (research_geodesic.rs:4817) the
  Seam path calls `ordered_wraps(.., spiral_levels + 1, ..)` with
  `spiral_levels = 0`, because only the Ramp join's `EntryLaw` sets it.
  So every closed wrap round the seed is lapped from a radial seam. The
  seam step reads about 0.94 D (the `Seam` doc, :1898), and the replay
  re-enters there. In 3D that is a lift-over and a plunge (R4 §1.2).
- `core_spiral` (:3391) exists and is called only under
  `JoinRule::Morph` (:4926):
  - It traces `F = T/s − φ/2π` out of the starter rim through the levels
    `1..=K`, where `K = field.star_levels(seed, r0)`: every level set
    closed round the seed and crossed once by each ray.
  - Then it adds one closing lap on level K.
  - Its chord step comes from the op tolerance (the `circle_from` rule).
    It drops its outer level and retraces until every segment lies inside
    the `ToolCentreRegion`.
- `ordered_wraps` already takes a first level (`min_level`), and the Seam
  path already starts it at the spiral's end point (`spiral.last()`), the
  hook the Ramp join used.

### 12.2 The design

A new option field, `GeodesicOptions::core: CoreJoin`, with two values:
- `CoreJoin::Seam`: today's behaviour, set in `G1` and `C2`. Every other
  preset inherits it, so every existing arm is unchanged.
- `CoreJoin::Spiral`: under `JoinRule::Seam` with no Ramp law,
  `plan_piece` calls `core_spiral` (with the same field, region, seed, the
  rim radius `r`, `phi0` from the starter end, `s` and the tolerance),
  emits the spiral first, from the starter end, through `append_path`, and
  calls `ordered_wraps(.., K + 1, ..)` from the spiral's end. The open-arc
  logic of the zigzag pick (turns, head-on ends, C28 / C29 pick) then
  starts at the first level past the core, unchanged.
- The new preset is `GeodesicOptions::C35_SPIRAL_PICK = { core:
  CoreJoin::Spiral, ..C28_ZIGZAG_PICK }`, and the new 3D arm value is
  `geodesic_local_spiral_pick` (`RESEARCH_ARM_ENV`).
  - `pick_variants` copies `core` into plan A and plan B (`..self`), so
    the C29 3D pick compares two plans that both cut the spiral.
  - C31 (the grown outline) is untouched: it acts on the outline before
    the producer.

The load, from the code's own derivations. The levels are the field's
(`LocalLevel` under these presets). The spiral moves out by one level step
per turn, so each turn meets the ring of the turn before it, one level
step out: the same band a closed wrap meets from the wrap before it. The
spacing law holds that band at or under c·D (C5, R4 §1.1). The seam step
(about 0.94 D) is gone. There is no new turn and no exemption.

**No new constant.** K comes from the field's star test; the chord step
from the tolerance; the levels from R, s, the ceiling and the tolerance
(C16); the reach from s and the cell.

**What the spiral does not cover:**
- closed levels past K (closed round an island, so not star-shaped from
  the seed). They keep the seam join.
- every open arc. These are R4 item 3, the wall-end descents.

### 12.3 The record and the test

- `SlicePlan` gains `core: Option<CoreZone>` (seed, radius `r +
  T_K · s`, levels K), set only under `CoreJoin::Spiral` with K > 0.
- `plan_one_outline` counts the 2D `Rapid`s of the cleaned plan that lie
  inside the core disc (radius − one cell), leaving out the piece's first
  `Rapid` (the starter entry at the seed). It records the zone with z and
  the count in `Research3d::core_zones` (new; the C29 pick drops the
  loser's records, as it does for turns).
- C12 heavy test: for each geodesic mode, print every core zone and count
  in 3D the lift-over descents into stock inside it: entry-plunge moves
  that drop more than 1 mm, at the level, more than R from the seed (the
  starter plunge is at the seed), and inside the zone's disc. For
  `geodesic_local_spiral_pick` (a new heavy mode) the test fails when the
  2D count or the 3D count is above 0, or when no core zone exists. The
  existing modes record no zone (`CoreJoin::Seam`) and print nothing new.

### 12.4 Byte parity

The shipped arms do not use `research_geodesic.rs`. Every existing
research preset has `core: Seam`, so `plan_piece` takes the old path;
`spiral` stays empty (or is the Ramp law's, as before). The new record is
filled only when a zone exists. The C29 pick and the C31 opening are not
changed. So `adaptive3d_emission_byte_parity` and the 2D render harness
stay byte-equal (the harness gains no column).

### 12.5 Pre-registered numbers (P; D20 is the base)

The counts are the README §4 instrument (`entries.py`): lift-over
descents into stock, which are keep-down-link spans whose plunge drops
≥ 1 mm and removes > 0.1 mm³, and the plunge-entry spans.

- **Corne Z 17**: 238 descents = 40 on the seam line + 198 at
  open-arc ends. The seam line runs from about 8.5 mm to about 47 mm from
  the seed, about one descent per ring. The nearest wall is about 33 mm
  away, so the closed, star-shaped core ends there: K reaches about ρ 33.
  - Expected removed: the seam descents with ρ < 33, about
    40 × (33 − 8.5)/(47 − 8.5) ≈ 25.
  - The seam descents past 33 mm (about 15) are on closed levels that are
    not star-shaped from the seed, or are not closed. They stay.
- **Corne, whole op** (1195 descents over 4 levels): Z 11 and Z 5 share
  the same seed and walls (C24: one piece holds all levels), so about −25
  each; Z 0.5 (the small-region grid) about −5. Expected about **1195 →
  1115 (range 1080–1150)**.
- **wanaka200** (3578): terrain levels have walls near every seed, so K
  is a few levels. Expected about **3578 → 3400 (range 3330–3550)**. Low
  confidence: I have no per-level seam count for wanaka.
- **Plunge-entry spans** (the G2 "plunges"): unchanged, Corne **60 ± 3**,
  wanaka **20 ± 2**. The spiral removes ring changes, not region entries.
- **The share of descents left at open arcs**: Corne about 198 / 213 ≈
  **93 %** at Z 17; whole op about **90–95 %**. wanaka about 95 %+.
  - This is R4's point (§6): the spiral alone fixes the closed rings, and
    the bulk is item 3.
  - If the share comes out much lower, then the open arcs were not the
    bulk, and my reading of the 198 is wrong.
- **Cycle time**: each descent removed saves about 0.55 s (a 6 mm plunge
  at 750 mm/min plus the rise). Corne about −45 s (2760 → about 2715),
  wanaka about −100 s.
- **C12 heavy test, new mode**: 0 2D Rapids and 0 descents inside the
  core zone at both levels; K ≥ 1. The pocket seed lies between the
  island and the walls, so K is small (a few levels). The exact-bound
  assertion holds (the spiral needs no exemption).

**Risk.** In C3 the Ramp join's spiral was refused by the replay and came
out "as 2 to 6 mm pieces". That spiral used plain spacing (s per turn),
which reads over c·D near the seed (the `Ramp` doc). `core_spiral` under
the curvature or LocalLevel levels is the case the spacing law covers.
If the replay still splits it, the 2D count in the core zone says so at
once, and the test fails there.

## 13. C37 design: no re-entry inside a connected zone, for open arcs (R4 item 3) (2026-10-26)

DESIGN ONLY; no code, no cargo. Inputs: R4 §1.2, §5 item 3; `P2_C2`
§C25 (the wall-end refusals); `P2_C10_leadin_guard.md` (the C8 guard,
found); C35 (§12) as the base this builds on.

### 13.1 Why the replay refuses the wall-end turns (Corne Z 17: 198)

The open-arc join today (`plan_piece`, the `C14` block; `JoinRule::Seam`
with `ArcLink::Zigzag`) plans a turn from the cutter (the end of arc k)
to the near end of arc k+1 when the turn is no longer than `turn_bound =
s + (1 + √2)·cell` and its chord lies inside the region; else the join is
a seam step or a link. The replay then turns a refusal into a 2D `Rapid`,
and in 3D a lift-over and a plunge. There are four ways to a refusal at a
wall, from the code and the notes:

1. **The head-on lens (C25, C23 problem 2).** An arc that ends square into
   a wall meets the lens of stock the arc before left at the wall:
   `arc_end_lens(R, Δ, θ)`, which at θ = 0 is `(R + Δ/2)/2R`, over the
   ceiling for every Δ. The replay refuses the last 0.6–2.1 mm of the arc
   and re-enters at the arc's end, that is, at the turn's start (C25: 17
   of the 19 lost turns on the C12 pocket).
   - On Corne the C29 pick kept plan A at every level (C29 §10d, D18).
     Plan A is plain zigzag, with the head-on rule off. So this mechanism
     is fully live at Corne.
2. **Over the bound at an oblique wall** (C14 (5)). Where the level sets
   meet the wall at θ off the normal, the arc ends are `Δ/cos θ` apart.
   Past the bound the turn is not planned; the seam step or the link that
   replaces it is refused.
3. **Outside the region at a convex wall** (C14 (5)). The chord between
   the two ends leaves the tool-centre region (round an island or a
   convex corner); the join is a link, and a link through standing stock
   is refused.
4. **A turn on a stub.** The shortfall and the smoothing cut short arcs
   near walls; an arc shorter than its turn gives no turn.

I cannot split the 198 between these four from the data in hand. `entries.json` has positions, not
reasons. The design therefore starts with the instrument (13.3). The
pre-registered split is in 13.4.

### 13.2 The C8 guard (found by C10) and what it means here

C10 (`P2_C10_leadin_guard.md`) traced the C8 lead-in, which applied 0
times:
- **Inside ρ\*** the curvature law spends the whole margin
  (`m = c·D − w ≈ 0`), so `lead_in_radius` gives none, or a radius ≈ ρ
  that guard #4 refuses. That is a limit by construction.
- **Beyond ρ\*** a mid-arc trace walks off the cut arc (guard #5). That
  is a bug, which hides the limit that the arcs are shorter than the
  span.
- C10's growing bite fits 0 bands at spacing `s`, and fits only with a
  reserved band margin (a spacing ruling).

**Consequence for C37.** On Corne (s 1.2) the margin beyond ρ\* is
`c·D − s = 0.113 mm`. A lead-in with a growing bite needs a lead length of
tens of mm at that margin. It is not the join to build. The join that
fits everywhere is the counted, fed step the operator already ruled for
(C14: length ≤ the bound, feed by the measured engagement,
`turn_feed_factor`), placed where it fits instead of only at the arc
ends.

### 13.3 The design

**The contract:** inside one connected zone of a level, the plan has no
2D `Rapid` after the zone's first entry. A zone is a connected component
of the field's reachable lattice at the level (`GeodesicField` reach). A
`Rapid` is allowed only between two zones the field proves disconnected.

The open-arc join, in order (the first that fits wins):
1. **The head-on rule, always** (`ArcEnds::HeadOn` with
   `MeetingRule::Through`, the C23 rule; no pick). Each arc starts at its
   end with the larger lens, and the counted turn cuts that lens. This
   removes mechanism 1 at its cause (C25: pocket 20 → 0 turns that start
   at an entry). The C28/C29 pick is then not needed (R4 §6).
2. **The zigzag turn** from the cutter to the next arc's start, when it
   fits the bound and the region (today's rule).
3. **The fed bridge** (new), when the turn does not fit:
   - The cutter returns along the band just cut (keep-down `Link`s, one
     per vertex: the C14 one-way return) to the point P of arc k from
     which the nearest point Q of arc k+1 is within the bound and the
     chord PQ lies inside the region.
   - It takes the fed step P→Q (a counted turn: the `LinkLoad::turns`
     exemption, the feed by measured engagement).
   - It cuts arc k+1 from Q to the end that holds the larger lens (the
     head-on rule's start), then returns along it (keep-down, through the
     band just cut) to Q, and cuts the rest of arc k+1.
   - Such a P exists wherever the two arcs are one level apart along
     their common length: across the band they stand `Δ ≤ s` apart, with
     the cell quantum inside the bound, by the derivation of `turn_bound`.
     So the bridge fails only where arc k+1 has no point within the bound
     of arc k: a branch, a stub, or a far side.
4. **A routed keep-down link** (R4 item 8, minimal form), when the bridge
   fails inside one zone: a shortest path over the cut lattice cells of
   the plan so far (the 2D grid the replay stamps), from the cutter to
   the next arc's start, emitted as keep-down `Link`s and entered by a
   fed step at its end. If no such path exists, the two arcs are in two
   zones of the cut set, and the `Rapid` is allowed. It is counted and
   classified.

**No new constant:** the bound (s, the cell), the ceiling, R, the
tolerance, and the lattice.

**The instrument, built first, in the same step:**
- the research record gains, per slice piece, the ArcReport counts
  (turns, over the bound, outside), plus the bridge and routed-link
  counts;
- every 2D `Rapid` after the first, per zone, is classified by the join
  that produced it (lens refusal, over the bound, outside, no bridge, no
  routed path).
This settles 13.1 on the first run.

**The sentry** (R4 item 3): the C12 heavy test counts the 2D `Rapid`s per
connected zone (the field's components at each level, from the record)
and fails on more than one per zone for the C37 arm.

### 13.4 Pre-registered numbers (P; base C35 as predicted in §12.5)

- **The split of the 198 wall-end descents at Corne Z 17:**
  - head-on lens (1) about 55 %;
  - over the bound (2) about 25 %;
  - outside (3) about 15 %;
  - stubs (4) about 5 %.
  Low confidence; the instrument decides.
- **Lift-over descents into stock** (README §4 instrument):
  - Corne 1195 (D20) → about 1115 after C35 → **about 150–350** after
    C37. What remains is the starter, the far-side and branch joins that
    need the routed link where it fails, and the F-038 and air splits of
    the 3D stage, which C37 does not touch.
  - wanaka200 3578 → about 3400 after C35 → **about 500–1200**. Lower
    confidence: terrain levels have many small zones, and between zones
    a `Rapid` stays legal.
- **Plunge-entry spans** (the G2 plunges): about unchanged (Corne 60 ± 5,
  wanaka 20 ± 3). Zone entries stay.
- **Counted fed steps (turns):** up by about the descents removed (Corne
  about +800 to +950). Each is a side cut at depth under the turn
  exemption. The G1 exact-bound assertion holds, because turns are
  exempt and fed.
- **Keep-down length:** up. The bridge returns along the band, about half
  an arc per bridge: Corne about +20–40 m over the op.
- **Cycle time:**
  - each removed descent saves about 0.55 s;
  - each bridge costs its return length at feed, plus about 0.1 s for the
    fed step.
  - Corne about −500 s + 300 s = **about −200 s** (2760 → about 2550).
    wanaka about −1200 s + 600 s = **about −600 s**. Low confidence; the
    C21 kinematic model decides.
- **Share of descents left at open arcs after C37:** under 30 % (from
  about 93 %).

### 13.5 Scope, risk and what I would build first

- Files: `research_geodesic.rs` (the join block in `plan_piece`, a new
  preset `C37_…` = C35 with the head-on rule and the bridge; the record
  types), the 3D record plumbing (as C35), and the C12 test (the sentry).
- **Risk 1.** The bridge's fed step P→Q is a radial step into an arc's
  interior. Its engagement is a cap like the turn's (0.75–0.94 D per
  step, C14 (7)), held by the feed, not by the ceiling. The exemption
  already covers it (≤ bound, fed), but it puts a load spike per bridge,
  as R4 §2.3 says of the fed step. C37 accepts that for open arcs. The
  spiral (C35) keeps it out of the closed core.
- **Risk 2.** The return-and-recut of arc k+1's near part doubles the
  path over that part. Bounded by the arc length; the instrument reports
  it.
- **Order:** the instrument and the head-on rule first (cheap, and they
  answer 13.1). Then the bridge, then the routed link. Each step is
  measured against the sentry.

### 13.6 Update: R4 §7 splits the wall-end descents; C37 against C38 (2026-10-26)

R4 §7.2 (from `entries.json`, D20, against the hole-filled silhouette):

| Plane | Descents | Seam line | Outer strip (outside the silhouette) | Inner walls |
|---|---|---|---|---|
| Z 17 | 238 | 40 | 124 (38 within 3 mm of the wall, 86 at 3–9 mm: the containment line) | **74** (6.5–7.0 mm inside the silhouette: the inner face of the frame wall, R from it) |
| Z 5 | 65 | 0 | 0 | **65** |
| Z 0.5 | 123 | 0 | 0 | **123** |

The 86 at the containment line end at a free edge. The C31 grow puts
`R − m` of phantom material beyond the real edge (11.3, "pessimistic,
not unsafe"), and the replay reads that band at the turn. Below Z 17 the
strip region is never planned (the F-038 forecast skips it before the
grow).

**The split of work:**
- **C38 (C12):** the F-038 skip, and the C31 grow at free edges (grow
  only along walls, or let a wrap end free at a material edge). It covers
  the 124 outer-strip descents. C31 is my code, so C12 asks me before it
  changes it.
- **C37 (this design):** the inner-wall open-arc ends, **74 + 65 + 123 =
  262** on Corne. R4 §7.2 gives no Z 11 row; I take its inner-wall count
  as one more set, near Z 5's, and report it when the instrument reads it.

**Where C37 touches C31 and C38 (so the two do not collide):**
- **C37 does not edit** `research_grown_outlines`, `smoothed_region`,
  `planner_outlines`, `grown_planner_outlines`, the F-038 forecast block
  of `clear_one_region`, or the opening record (`RegionOpening`). Those
  are C38's (with my review, since C31 is mine).
- **C37 reads their output only through the 2D outline** that
  `plan_one_outline` passes to the producer. The fed bridge's "chord
  inside the region" test, the head-on rule's lens test and the routed
  link's lattice all read the piece's own `ToolCentreRegion` (built in
  `plan_piece` from that outline). So when C38 changes the grow at free
  edges, C37 sees the new region with no code change.
- **The one shared file is `adaptive3d/clearing.rs`, in disjoint
  places.** C37 adds its record plumbing next to the C35 core-zone record
  (`plan_one_outline`, after the cleanup; the C29 pick's record ranges).
  C38 edits the opening path before the producer. A merge touches
  neither of the other's lines. I will tell C12 the exact line ranges
  before stage 1.
- **The zone sentry depends on C38 in one way.** The zones are the
  field's reach components at the level, and the field is built on the
  outline C38 decides. A free edge that C38 stops growing changes the
  zone's shape, not its count. The sentry reads the zones from the record
  of each run, so it follows C38 with no change.
- **Order.** If C38 lands first, the stage-1 instrument reads the inner
  set alone. If C37 stage 1 lands first, its instrument classifies the
  outer-strip refusals too (as "outside" or "over bound" at the
  containment line), and the D-run splits them out by R4's silhouette
  test.

**Pre-registration for the inner-wall set only (P; replaces the 13.4
descent numbers; base C35 as in §12.5):**
- **Mechanism split of the 262**, by plane:
  - Z 17 (74, the inner face of the frame wall, arcs end square into it):
    head-on lens about 75 %, over the bound about 15 %, outside about 5 %,
    stubs about 5 %.
  - Z 5 (65), the same walls: about the same split.
  - Z 0.5 (123, the grid of small regions and hex holes, convex walls):
    head-on lens about 40 %, outside about 30 %, over the bound about
    20 %, stubs about 10 %.
- **After stage 1** (the instrument and the head-on rule always on):
  262 → **about 110–160**. The lens class goes; the C25 pocket went
  20 → 0 under the same rule.
- **After stage 2** (the fed bridge): → **about 30–70**. The over-bound
  and outside classes go where a bridge point exists.
- **After stage 3** (the routed link): → **about 10–30**: true zone
  changes and joins with no cut path. They are legal `Rapid`s under the
  contract, and the sentry counts them per zone.
- **Corne whole op** (lift-over descents into stock), when C35, C37 (all
  three stages) and C38 have landed: 1195 → about 1115 (C35) → about
  **60–200**. The outer strip goes to C38: its 124, and the same strip at
  Z 11 if planned, are its number to pre-register.
- **Plunge-entry spans:** about unchanged (60 ± 5).
- **Stage 2 report items, as the lead asked:**
  - the count of fed-bridge steps (expected about one per removed
    over-bound or outside descent: Corne about 60–110);
  - the feed dip per bridge: the factor `turn_feed_factor` and the feed
    in mm/min, with its quartiles. The factor `c / max(w, a)` for a
    head-on cap of 0.75–0.94 D at Corne's c 0.219 is about 0.23–0.29,
    so F 4000 → about 930–1170 mm/min (R4 §2.1);
  - the spike length per bridge (≤ the bound: `s + (1 + √2)·cell` =
    2.41 mm at Corne).
- **wanaka200:** R4 §7 does not split it. I keep 13.4's range for the
  whole op and will split it by the instrument in stage 1.

**Hold.** No file in `adaptive-head` changes until the lead reports D21
(C35) done and committed.

### 13.7 Scope change: C37 also takes 79 outer wall-end descents (C38 §5–§6) (2026-10-26)

C12's `P3_C38_outer_band.md` §5.2 (script `phase3/c38/classify_outer_descents.py`,
asserts every number) splits the 124 outer-strip descents at Z 17 by the
edge beyond them:

| Class | Count | Owner |
|---|---|---|
| beyond the mesh bbox, in the 5 mm margin, 0.55–4.6 mm from the stock-box edge (air beyond: a true free edge) | 45 | **C38** (clip the grown outline to the stock box at the call site) |
| on the part grid, 1.5–3 mm from the silhouette (wall ends, centre at the R limit) | 6 | **C37** |
| on the part grid, 3–4.5 mm from the silhouette (wall ends at wall + R) | 58 | **C37** |
| on the part grid, 4.5–6 mm from the silhouette (near the containment line, with real stock beyond) | 15 | **C37** |

**The containment line is not a free edge.** C38 §5.1 shows that real
stock stands beyond it (the O5b clear makes the planner stock read air
there). So my §13.6 note, which took R4's "free edge" reading, is
corrected. C37 owns the open-arc join contract at every wall, outer and
inner, and at the containment line: **262 + 79 = 341 descents on Corne**.

**The instrument uses C12's silhouette rule,** so that C37's and C38's
counts add up to the D20 totals:
- C37's per-join classification (lens, over the bound, outside, stub)
  carries a second key, the edge class of its position: the four C38
  classes (beyond the mesh bbox; 1.5–3, 3–4.5, 4.5–6 mm from the hole-filled
  silhouette on the part grid) plus "inside the silhouette".
- The research record gives each refusal its position. The D-run applies
  `classify_outer_descents.py`'s rule to the record's positions with the
  same part grid (`fix9/corne_part.npz`). No new rule in the Rust code.

**Pre-registration for the 79 outer wall ends (P; they join §13.6's set):**
- The mechanism split, by class:
  - 64 wall ends (1.5–4.5 mm): the same mechanism as the inner face of the
    frame wall, so lens about 75 %, over the bound about 15 %, outside
    about 5 %, stubs about 5 %;
  - 15 near the containment line: no wall stands there, so a head-on
    approach meets the real band beyond the line (C38 §5.1), which the
    replay reads as stock. Lens about 30 %, over the bound about 40 %
    (the line meets the level sets at an angle), stubs about 30 %.
- After stage 1: 79 → about 35–50. After stage 2: about 10–25. After
  stage 3: about 5–15.
- **The full C37 set (341):** 341 → about **145–210** after stage 1 →
  about **40–95** after stage 2 → about **15–45** after stage 3.
- **Corne whole op, with C35, all three C37 stages and C38 item B** (the
  45): 1195 → about 60–200, as in §13.6. The 79 were already inside the
  outer strip I had counted there; the total does not move, only the
  owner.
- **Two caveats tied to C38:**
  - C38's P-B2 (the 79 change by at most 25 % under item B) is C38's
    pre-registration. Mine above is for C37's stages, read after item B
    lands. If item B lands first, my stage-1 base is C38's measured 79′
    rather than 79.
  - C38's corridor opening (its §3.2, behind a ruling) adds new descents
    below Z 17 (a slot entry per corridor piece per level). Those are
    entries of a newly planned region, not open-arc ends. My sentry
    counts them as the zone's first entry, not as a violation.

C12 will send its line ranges and will ask before it touches the C31 call
site (`research_grown_outlines`). I still hold all code until the lead
reports D21.

### 12.6 D21 RED: the replay splits the core spiral (8 re-entries per level); the instrument (2026-10-26)

D21 (`phase3/d21/10_c12_heavy.txt`):
- core at Z 7.00 and Z 4.50: seed (-18.46, -8.88), radius 9.31 mm, 4
  levels; "2D re-entries 8, 3D entries inside 8" at each level;
- 0 over the exact bound; the turns are identical to `zigzag_pick`;
- plain test: 55 entries against 47 for `zigzag_pick`.
This is the risk of §12.5.

**What the geometry says (computed, s 2, R 3, c·D 2.1759).** The
curvature law places the levels at ρ = 3.000, 4.437, 6.031, 7.733, 9.514
(steps 1.437, 1.594, 1.703, 1.781). On turn n the spiral runs from level n
to n+1 while the turn before it ran from n−1 to n. So at the fraction f
of a turn the band is `d_n + f(d_{n+1} − d_n)`, at a pass radius between
the two levels. Over f in [0, 1] the circular-front width
`w(ρ, Δ) = R − (a² − R² − ρ²)/(2ρ)`, `a = ρ − Δ + R`, never exceeds c·D
on turns 1–4: the excess is 0 at f = 0 (exactly on the law) and negative
between. Turn 0, against the starter lap, is far under. **So on exact
circles the spiral holds the law.** Its spacing does not exceed the
curvature law anywhere. The refusals come from what the exact model
leaves out.

**The candidates (none confirmed without a run):**
- **H1, a zero margin read on the lattice.** At the law the pass reads
  exactly c·D (C10: `m ≈ 0` inside ρ\*; C24: LocalLevel's bisection lands
  up to 0.075 mm under the root). The replay reads the swept width on the
  planner lattice (cell 0.5), whose quantum is one cell over D (0.083). A
  pass placed exactly on the ceiling can read one quantum over where the
  spiral crosses the lattice badly. Two places per turn would fit that.
  The closed rings of `zigzag_pick` sit on the same law, so H1 must also
  say why they pass. Their polylines are lattice contour chains, while
  the spiral is a polar curve with chords from the tolerance.
- **H2, not the spiral.** The `Rapid`s inside the disc may enter another
  producer: a residue contour or a mop patch that the spiral leaves work
  for (for example between the starter lap and the first turn), not a
  split of the spiral.
- **H3, the spiral's discretisation.** Its chord step comes from the
  tolerance at the outer radius (22 chords per turn here, 2.66 mm at
  ρ 9.3, sag 0.095). I showed the radial alignment of the vertices keeps
  the band under Δ at chord midpoints, so H3 is unlikely. It stays listed
  until the readings rule it out.

**The instrument (built; no change of behaviour):**
- `CoreZone` now carries the ceiling and every RAW-plan step over the
  ceiling with an end inside the disc. That is
  `pass_load_readout(raw, parts)` in `slice_segments`; the raw readout's
  over-steps are the steps the replay refuses. Each step carries its
  radii from the seed, its reading, its `StepPart` (Spiral, Wrap, …) and
  its producer.
- `core_rapids` now returns each `Rapid` inside, with the producer it
  enters.
- `CoreZoneRecord` gains `ceiling`, `rapid_lines` and `over_lines`, and
  the C12 tests print them under each core line.
- So one run of the plain test shows every split: where it is, the swept
  width there against the ceiling, and whether it is the spiral (H1/H3)
  or another producer (H2).

**No fix is built yet.** "Fix the cause, not the count", and the cause is
not proven. The fix by outcome:
- **H2:** the spiral leaves a gap that another producer refuses. The fix
  is in the spiral's start or closing lap.
- **H1:** the spiral needs the margin the closed rings get from their
  lattice tracing. The derived fix is to trace the spiral's chords at the
  lattice cell instead of the tolerance, so it reads like a contour chain.
  Or the spiral steps one bisection bracket under the law; the bracket is
  the LocalLevel tolerance, not a new constant.
- **The core too small:** if the readings show over at f ≈ 0 on every turn
  (on the law itself), then a 9.3 mm core at s 2 sits wholly inside ρ\*
  (22.7 mm) with no margin. C35 would then need the spacing one quantum
  under the law inside the spiral only: a cost of about one extra turn in
  a 4-level core.

### 12.7 D21b: the cause is the chord heading (H3, not the sag); the fix (2026-10-26)

**The readings** (`phase3/d21/b_3_c12_plain.txt`): 9 raw overs, all
Spiral / Clearing, 0.3631 to 0.3931 against 0.3626. The 8 rapids are the
ends of the first 8 overs. H2 is out.

**Each over is the second half of a chord.** The tolerance rule gives 22
vertices per turn (`dφ` 0.286 rad, chords 1.2 to 2.7 mm). The replay
splits each chord into steps of at most 1.5 mm, so a chord is two steps
of about 1 mm. The chord's middle lies 0.1 mm inside the circle (the
sag), so its second half runs out by about 0.1 mm. That is the radial
change of 0.06 to 0.15 mm that the readings show on each over.

**Why a step of 1 mm reads up to 8 % over.**
- The vertices ARE radially aligned with the turn before. Each turn has
  the same 22 rays (`θ = j·dφ`). The alignment holds the gap; it does not
  hold the heading.
- The swept width reads the extent of the stock in the disc ACROSS the
  step's heading. At the end of a chord the heading is `β = dφ/2` (0.143
  rad) off the tangent, out into the stock. The cap of stock ahead of the
  cutter then reads `R(1 − cos β) + h sin β` wider (`EntryLaw`'s
  tilted-step bound, `h = √(R² − (R − s)²)` = 2.83). The previous turn's
  chord ahead also lies `ρ(1 − cos β)` inside the level. At 22 per turn
  that is 0.43 + 0.09 mm, about 0.08 of D.
- The law has no margin inside `ρ*`. So any tilt reads over on exact
  geometry. The lattice reads up to one cell short, so only some steps
  show (9 here, at no fixed place on the turn).
- My §12.6 model checked the gap at the chord midpoints, not the heading.
  The note in `transition_fraction` ("the spiral form has no kink") is
  true for the curve only.

**The model asserts this** (`phase3/d21/c35_chord_model/assert_chord_tilt.py`,
exact circles, the replay's step reading):
- On a fine grid (0.05) at the law levels the excess is 0.077, 0.0165
  and 0.0037 of D at 22, 88 and 352 vertices per turn. It is linear in
  `β` and goes to 0, so the law itself holds on the curve.
- At 22 per turn on the 0.5 lattice: 12 to 18 overs, max 0.40 to 0.43,
  at each of five lattice phases (Corne: 9, max 0.393).
- The held spiral below: 0 overs at every phase, max 0.325 to 0.337; on
  the fine grid 0 overs, max 0.344.

**The fix (built; research arm `geodesic_local_spiral_pick` only):**
`held_core_spiral` in `research_geodesic.rs` replaces `core_spiral` on
the C35 path. `core_spiral` (morph arms) gives the same output; its
tracing moved into `trace_core_spiral` and `chord_turn_steps` with the
same arithmetic.
1. The angle step holds the tilt to the op tolerance:
   `β = atan2(R, h) + asin((tol − R)/√(h² + R²))` (`held_tilt`, the form of
   `transition_fraction` with margin = tol). At s 2, R 3, tol 0.1:
   β 0.0347, 91 vertices per turn (the chord rule is used when finer).
2. Each vertex radius is bisected to the float resolution (tolerance 0 in
   `radius_at`). A radius error of 0.1 mm between two vertices 0.5 mm
   apart would tilt the chord by 0.2 rad.
3. The turns are spaced by the LocalLevel walk
   (`GeodesicField::spiral_turn_levels`) at the ceiling width less the
   tilt term (0.099), the chord sag at the outer radius (0.006) and one
   tol (the law is read on the traced contours, the spiral on the
   bilinear field). Width 1.971 against 2.176. The last turn ends on
   level K, so the wraps and the zone radius do not change.
- Every number comes from R, s, tol and the ceiling. No new constant.

**Cost.** On the model core: 5 turns instead of 4 (the last one 0.77 mm),
one extra lap of about 60 mm per core. The P numbers of §12.5 move by less
than their bands: Corne about +1 lap × levels with a core.

**What D21 must show:** the spiral arm's core zones have 0 re-entries and
0 raw overs inside the disc; every other mode prints as in D21b
(`core_spiral` is unchanged for the morph arms; the seam arms do not call
it).

### 13.8 Built after the C35 commit (3cc257c1f): the C38 slot types and C37 stage 1 (2026-10-26)

**C38 slot types (agreed in C38 §6.1, item 4):**
- `SlicePlan.slots`, `TurnKind { Turn, Slot }`, and `SliceTurns.planned_slots` / `kinds`.
- `slice_turn_records` returns each record with its index into the list (turns, then slots).
- `SliceTurns::mark` / `truncate` / `drain` replace the hand-kept counts. `pick_region_plan` and C12's `RecordMark` use them.
- `apply_slice_turn_feeds` returns `(turns, slots)`, and `Research3d.slots` holds the slots.
- With no slot, every list and every record is as before.

**C37 stage 1 (the instrument and the head-on rule; no bridge, no routed link):**
- **Arm:** `geodesic_local_spiral_headon` (`GeodesicOptions::C37_SPIRAL_HEADON`). It is C35 with `ArcEnds::HeadOn`, `MeetingRule::Through` and no pick, that is, C35's plan B on every piece.
- **Join marks:** `plan_piece` records each join into a wrap as `JoinMark { from: the cutter, to: the wrap's start, class }`. The classes are:
  - `Start` and `CoreHandOff` for the first wrap;
  - `ClosedSeam` and `Transition` for two closed wraps;
  - `Mixed`;
  - `Seam`, `Turn`, `OverBound` and `Outside` for two open arcs, set where the C14 counts are set;
  - `Corridor`, set by C38.
  The marks do not change the plan.
- **Re-entry classes:** `classify_reentries` labels each 2D `Rapid` of the slice plan that goes to the 3D stage. It skips the first `Rapid` and works after the F-038 filter. The label is the class of the nearest mark whose step lies within `turn_bound` of the `Rapid`, or "no join". The bound is the plan's own; no new constant.
  - A re-entry at a `Turn` mark is the head-on lens refusal (mechanism 1 of §13.1).
  - At `OverBound` or `Outside` it is mechanism 2 or 3.
  - "No join" is a split inside a wrap or a cleanup producer's entry.
  Each re-entry also carries its producer.
- **The record:** `Research3d.joins` holds one `JoinsRecord` per field-planned slice piece: the `ArcCounts` and the re-entries with their positions. It is rolled back with the pick and with C38's region judge.
- **C35b:** the core-zone record gains `handoff_line`. It gives the hand-off step, its largest raw over-reading, and the 2D re-entries from one planner cell inside the core's radius to one stepover outside it.
- **The test:** the C12 tests add the arm, print `join_lines` for every mode, and print one `c37 re-entry` line per re-entry for the D-run's edge classes (C38's silhouette rule). Stage 1 asserts nothing new. The core-zone sentry applies to the new arm, because it is a spiral arm.

**What the stage-1 run must show (P, from §13.6 / §13.7):**
- On the C12 pocket, the head-on arm's re-entries at `Turn` marks fall against `geodesic_local_spiral_pick`'s. C25 saw 20 → 0 on the pocket under the same rule.
- On Corne, the 341 → about 145–210 after stage 1 is read by the fixture run with the `c37 re-entry` lines.

### 12.8 C35 on wanaka200 (fix12): why the descents fell by 1562, why the over-load rose, and why the Z 4.0 openings changed (2026-10-26)

Read only: the fix12 and fix7 artefacts, with python. Scripts are in
`phase3/fix12/c22_wanaka/`. `assert_12_8.py` asserts each claim below; run
`descents_by_core.py`, `over_by_core.py` and `over_vs_descents.py` first.
D20 is the fix7 `zigzag_pick` run, C35 the fix12 `spiral_pick` run.

**1. The 1562 descents.**
- **The core.** At Z 19.54 and 14.08 the one region is the 200 × 200 block (40 099.9 mm²), with the seed at (119.80, 124.87).
  - C35 leaves the seed as ONE spiral of about 58 turns. It runs out to r 97.4 mm, where it meets the block's edge (y 221.74), with no radial step on the way (2382 G-code moves).
  - D20 makes its first radial seam step at the first ring (r 3.00 → 4.38).
  - So the core at those levels is the whole inscribed disc. Its K is about 55 levels, not the 4 of the C12 pocket.
- **Descents** (fix9/entries.py rule), split by the disc r < 97.4 mm:

| Level | D20 inside | C35 inside | D20 outside | C35 outside |
|---|---|---|---|---|
| Z 19.54 | 48 | 0 | 77 | 80 |
| Z 14.08 | 48 | 0 | 77 | 80 |
| Z 8.62 | 2430 | 995 | 898 | 861 |
| Z 4.0 | 0 | 0 | 0 | 0 |

- **The seam mechanism** that §12.5 predicted shows at Z 19.54 and 14.08: one descent per ring, 48 per level, all removed. At Z 8.62 it gives about the same 48. That is about 150 in all, in line with the predicted −180.
- **The other 1435 are at Z 8.62, inside the disc, and they are a different mechanism.** There the path drapes over the terrain. The descents end at Z 9–13, not on the plane.
  - Each one is a short lift-over ALONG the path: the tool lifts, hops a median 0.79 mm (D20) or 1.72 mm (C35) with no radial change (dr ≈ 0), and drops a median 4.3 mm.
  - Per clearing move inside the disc they occur at 60.7 per 100 moves on D20 and 29.9 on C35.
  - The counts by 10 mm radius bin fall by about the same share from r 10 to r 100. Beyond r 100 they are identical in the two runs, where the plans are the same.
  - This is the 3D stage's lift-over over terrain, not a 2D re-entry. I did not predict it, and I cannot prove its cause from the artefacts. The C37 instrument (D23: the 2D re-entry classes) separates the two: a hop that is not a 2D re-entry is the 3D stage's. My reading: the spiral's chords are longer than the lattice-traced rings' moves (median hop 1.72 against 0.79 mm), so the 3D stage finds fewer short stretches to lift over. It is a by-product, not something C35 set out to do.

**2. The over-load (+506 mm by the lead's table; +502 mm by my count over all clearing cuts).**
- **Not in the core, not at the hand-off.** Over 0.471 is 0 mm at Z 19.54 and 14.08 in BOTH runs. Those levels hold the whole spiral, and the spiral ends at the wall there, so there is no radial hand-off step.
- **All of the rise is at Z 8.62, inside r 105 mm.** D20 has 1493.5 mm there and C35 2005.9 mm. Beyond r 105 the length is the same (199.8 mm).
- **Where the rise lies:**
  - **+288 mm** lies within R of the 433 places where D20 lifted over and C35 does not (D20 descents with no C35 descent within 3 mm): 347.1 → 635.5 mm. The tool now stays down and cuts through the stock that D20 lifted over.
  - **The other +224 mm** grows as the cut length does. The cut length inside r 105 at Z 8.62 rises by 16.3 % (14 038 → 16 326 mm), and the over length away from those places by 19.5 %. The held spiral spaces its turns at the width 1.971 mm against 2.176 (§12.7), so it cuts more turns over the same disc.
- **So it is not the C35b hand-off slot.** It is the drape at Z 8.62 cutting through stock that D20 lifted over, plus the extra turns. I did not split the thick part (+94 mm) by place. I expect the same split, but that is not measured.

**3. The Z 4.0 openings.**
- The regions of a level are not fixed. `detect_and_order_regions` builds them from the PLANNER stock after the levels above (`build_material_bool_grid(material_stock, ..)`, clearing.rs:1919–1949 at 3cc257c1f). C35 cuts Z 8.62 differently, so the stock left over Z 4.0 differs, and so do the regions.
  - Region 1: 2580.5 → 2563.2 mm².
  - Region 3: 159.9 → 160.4 mm².
- On the new outlines, C31's shortfall step takes other branches:
  - Region 1 was "closing guard" in D20, with a centre area of 77 602.8 mm² against a target of 1157.2. That is the G-RINGGROW fault: the ring's centre grew out to the box. It is now "shortfall inset split" (centre 1228.2).
  - Region 3 was "split" and is now "grown, no sweep".
- So C35 does not change the regions directly. It changes the stock above them, and G-RINGGROW (parked) flips its branch on a change of 17 mm² of outline. The flip is an effect of that parked defect, not of C35.

**What follows for C37 / D23:**
- The D23 2D re-entry classes on wanaka should show the Z 8.62 hops as NOT 2D re-entries, if my reading of item 1 is right.
- If they are 2D re-entries, my reading is wrong, and the 2D plan at Z 8.62 differs between the arms in a way I have not found.

## 14. C22a design: held links over ground that the F-038 filter dropped (2026-10-26)

DESIGN ONLY. No code until D23 is done and the lead approves. Input: C38
§9.4 (committed 382b4b8e7) and the D22 run in `phase3/c38/corne_c38a`.

### 14.1 The cause, by the code order

In `plan_one_outline` (research arm, `KeepDownLinks::WithinPassLoad`):
1. **The research cleanup** (`slice_cleanup` → `cleaned` →
   `apply_residue_mop_cleanup_turns`) ends its own short-cut filter with
   `replay_kept_segments` (`adaptive/path.rs:641`, called at :1823). That
   replay walks the kept plan again on a fresh grid:
   - every `Link` is judged again on the grid of that moment, and a refused one becomes a `Rapid`;
   - every `Cut` is re-walked under the pass load, so a cut that stood on material a dropped cut had cleared is split where it would overload.
   The doc comment states the rule: "a cut the filter left standing on
   material a dropped cut had cleared is split where it would overload".
2. **Then the 3D F-038 group filter** (`clearing.rs:2500–2575`) drops each
   group (a run from one `Rapid` to the next) that cuts less than
   `min_region_cut_length_mm`. The copy op dropped 509 of its 610 groups.
   **No replay follows.**
3. **`HeldLinks`** emits each `Link` as a keep-down link at the level after
   a Z-only check. The sink stamps it.

So the 3D filter is a second short-cut filter that skips the step the 2D
cleanup applies after its own filter. A `Link` admitted over a dropped
group's ground is now a link through stock: the D22 bar, 301 link samples
on 193 moves, up to 15.43 mm. The same holds for a `Cut` after the
dropped group. The replay read its load with the dropped group's stamps
in the grid, so it can now cut the dropped group's stock at more than the
ceiling. The links show it first because a link has no load reading in 3D.

### 14.2 Which fix is right at the cause

- **"Run the group filter before the replay": not possible as stated.**
  The groups ARE the replay's output: each `Rapid` that splits a group is
  a refusal of the replay. Before the replay, a piece is mostly one group,
  and the filter has nothing to judge.
- **The stock test at emission (C38 §9.5 item 1): a guard, not the fix.**
  It turns the bad links into entries. It leaves the cuts of 14.1 admitted
  on ground that was never cut, and it hides the divergence instead of
  closing it.
- **The fix at the cause: replay after the filter, the rule the 2D cleanup already follows.**
  - In the research arm, after the F-038 filter drops one group or more, the kept plan goes through `replay_kept_segments` again, with the same grid set-up and the same `LinkLoad` (the turns and slots included) as the cleanup's replay.
  - Links over dropped ground are refused there (→ `Rapid`), and cuts over it are split under the pass load.
  - The new `Rapid`s make new groups, so the filter runs again on the replayed plan. This repeats until no group drops.
  - The loop ends: each round drops at least one group or stops, and the number of groups is finite. In practice I expect one or two rounds.
  - **The code:**
    - one `pub(super)` wrapper in `adaptive/path.rs`, `replay_filtered(polygon, params, segments, measure, turns)`. It holds the set-up lines of :1800–1822 and the call. It is new code that no shipped path calls, so shipped output stays byte-identical.
    - a `pub(crate)` door in `research_geodesic.rs`;
    - the loop around the F-038 block in `plan_one_outline`, under `cfg(feature = "research")` and `KeepDownLinks::WithinPassLoad` only.
  - **The F-038 block itself:** I would move it into one function that both arms call, so the loop does not copy it. It is shared code (C12 edits next to it), so I coordinate with C12 first.
  - No new constant. The replay is the existing one, and the bound and the threshold are the existing ones.
- **C38 §9.5 item 2 (no F-038 group filter inside a research region) also
  fixes the cause.** With no filter, the plan the replay read is the plan
  that goes out. It costs every short group its entry (up to 509 in the
  copy op, plus 37 per level in op 1's strip). Filter-then-replay keeps
  the filter's saving, and it costs only the links and cuts that really
  crossed dropped ground. Both are correct, so the choice is a cost
  question. The lead decides; my design is filter-then-replay.
- **No stock test at emission now.** After the fix the 2D grid is the
  emitted plan, and a 2D grid starts full, so a held link that the replay
  admits crosses ground the emitted plan cut. If the counter below reads
  more than 0 after the fix, a third divergence exists, and the guard is
  then the right extra step.

### 14.3 The sentry

- **The invariant, measured where it can break:** a counter in the research
  record. `HeldLinks` emission reads the PLANNER stock along each held
  link's swath, using the test `plan_entry` makes:
  `max_conservative_top_z_in_disc` against the level plus
  `stay_down_clearance_mm`, at each held point. The counter counts the
  links that read stock there. Research only, recorded in `Research3d`,
  with one `info` line per offending link for CLI runs. It changes no move.
- **The test:** a fed link must not remove stock in a research run.
  1. A lib test in `research_geodesic` that builds the case in 2D:
     - a plan of three groups: a group cut A of 1 mm (under the threshold), a `Link` across A's ground, and a cut B;
     - run the research filter-and-replay;
     - assert that the link became a `Rapid` and that no step of B reads over the ceiling on the replay grid.
     It needs no 3D.
  2. The C12 heavy test sets `min_region_cut_length_mm > 0`, so the
     filter runs. For every research arm it asserts:
     - the held-link stock counter is 0;
     - no link move removes more than the volume quantum in the dexel simulation, the D22 bar's rule.

### 14.3b Agreed with C12 (2026-10-26)

- C12 makes no edit in the F-038 block or next to it. The block moves,
  unchanged, into `filter_short_groups`.
- C38's G-b judge reads the plan after the filter-and-replay loop, which
  is the plan the region emits. It rolls back every research record the
  region pushed.
- The re-replay uses the cleanup's own turn list,
  `research_turns = turns ++ slots`. Otherwise the second replay refuses
  the corridor slots, their groups split and drop, and the strip comes
  back.

### 14.4 Pre-registration (P; base D23 if it lands first, else D22 c38a)

- **Corne copy op:**
  - The D22 bar: 301 samples on 193 moves → **0**. The held-link stock counter: **0**.
  - Copy-op entries: **+50 to +200**. The at most 193 refused links become `Rapid`s, some cuts split, and the second filter round drops some of the new tiny groups.
  - Copy-op cut length: **−0 to −5 %**.
- **Corne op 1:**
  - D20 had 0 bar samples, so the change is small: entries ±20, over-load 0.471 within ±2 %.
  - If the G-b judge (C38) has replaced the F-038 filter in research regions, op 1 does not change at all.
- **wanaka200:** entries within ±1 %, cycle within ±1 %. The filter drops few groups there.
- **The replay rounds:** at most 2 per outline on both fixtures. More than 3 means the filter and the replay feed each other, and I report it.
- **If the counter is > 0 after the fix:** a third divergence exists. Then I add the emission guard of §9.5 item 1, with its own count.

### 13.9 D23: the stage-1 result explained (2026-10-26)

Read only. Scripts are in `phase3/fix13/c22_headon/`: `headon.py` and
`time_split.py`, then `assert_13_9.py`, which asserts every number below.
The base for the head-on arm is **D23 spiral_pick** (the same build), not
C35. The D23 build also holds C38.

**1. The 2D re-entries fell; the 3D descents did not.**

| Corne op 1 | D23 pick | D23 headon |
|---|---|---|
| 2D re-entries (C37, kept plan) | 161 | 120 |
| keep-down descents ≥ 1 mm that remove stock | 1010 | 1093 |
| keep-down descents ≥ 1 mm through air | 2 | 180 |
| keep-down link XY length | 5397.5 mm | 11 948.1 mm |

- Of the 1093 head-on descents, only **188** lie within R of a 2D re-entry
  at their level. The other ~905 come from the 3D stage, not from the 2D
  plan. They are the drape and large-Z splits over the part's top faces,
  as on wanaka (§12.8). Only 419 of the 1093 end on a level plane.
- So my §13.6/13.7 pre-registration counted the wrong thing. It assumed
  that one 3D descent is one 2D re-entry. On Corne the 2D re-entries are
  at most about 1 in 6 of the descents (188 / 1093); on wanaka about 1 in
  12 (164 / 2016).
- **Why the head-on arm has more descents:** the head-on rule starts each
  arc at its lens end. When the straight step from the cutter does not
  fit, the zigzag join takes the C23 one-way return. That is a keep-down
  link back along the arc just cut, one link per vertex, split at the
  planner cell. This adds 6.55 m of keep-down link on Corne. Where the
  return or the turn after it crosses a part face above the level, the 3D
  stage lifts it and drops again: +178 descents through air, and +83 that
  remove stock. I did not prove which return makes which descent; the
  link length and the air descents are measured.

**2. Plunge-entry spans:** 74 against 67 (D23 pick) and 60 (C35). The
C35 → D23 pick rise of +7 is C38's band and corridor openings. The head-on
rule adds 7 more. I did not split those 7.

**3. Smoothness:** the planar turns over 30°, read at the move junctions
of the simulation samples, coarser than the G-code audit:
- head-on 799 against pick 496 (+303);
- of those, junctions that touch a link move: 216 against 16 (+200). These are the returns: each one reverses at the arc end and turns again at its own end.
- clearing to clearing: +103.
The G-code audit's 1468 against 1063 (C35) shows the same rise. So the
head-on rule as built makes the path less smooth, through its returns.

**4. Cycle time (Corne, simulated, by op and intent):**
- C35 → D23 pick: +174.6 s. Op 1 is +16.5 s; the copy op is +158.1 s. That is C38's band and corridor cutting, nearly all in the copy op.
- D23 pick → D23 headon: +266.9 s, all in op 1 (the copy op differs by 0.5 s):
  - link time +127.7 s (the returns);
  - plunge time +78.3 s (the air and stock descents);
  - cut time +59.3 s.
- So of the 3199 s against 2757 s, about 175 s is C38 and about 267 s is the head-on rule.

**5. wanaka: no change.** 153 of its 164 2D re-entries are OverBound, 54 at
each of Z 19.54, 14.08 and 8.62. They are the open arcs in the block's
corners, outside the core disc of r 97.4, where the level sets meet the
walls at about 45°. So the arc ends lie `Δ/cos θ` apart, over the bound.
The head-on rule does not touch that mechanism. The 2016 descents are
the 3D stage's hops of §12.8, the same in both runs.

**What this means for stage 2 (the fed bridge):**
- The bridge removes OverBound and Outside 2D re-entries: Corne 55, wanaka 153 at most.
  - On Corne, each 2D re-entry carries about 1.6 descents near it (188 / 120). So the bridge can remove at most about 90 of the 1093.
  - On wanaka at most about 153 of the 2016.
  - The §13.7 targets (341 → 40–95 after stage 2) are therefore out of reach by the bridge. They assumed 2D re-entries are the descents.
- The large share of descents is the 3D stage's: the drape and large-Z splits and their lift-overs. A bridge in the 2D plan cannot remove them.
- **My recommendation:**
  1. Do not build stage 2 yet. First an instrument on the 3D stage: each 3D descent with the cause that made it (the 2D re-entry, the large-Z split of a draped cut, the C26b lift of a held link, the air split, F-038). The same per-cause count that C37 stage 1 gave for the 2D plan.
  2. Do not keep the head-on rule always on as built. It removed 41 2D re-entries and cost 267 s, 180 air descents and about 300 sharp turns, through the one-way returns. Either keep the C29 3D-scored pick, which chose plan A almost everywhere on Corne, or keep the head-on start without the per-vertex return (the turn from the cutter only). That variant is a question for the lead.

## 15. C40 design: the cause of every 3D descent in a research run (2026-10-26)

DESIGN ONLY. No code until the lead approves. Input: §12.8 (wanaka) and
§13.9 (Corne). Both found that most 3D descents are not 2D re-entries.

### 15.1 Where the 3D stage makes an entry (read from the code)

Every descent the fixtures count is an entry: a keep-down-link span or a
plunge-entry span that ends in `EntryPlunge` / `EntryHelix` / `EntryRamp`
moves. Each one starts as an `Adaptive3dSegment::Rapid` that
`plan_and_emit_region` hands to the sink (`sink.emit`, clearing.rs). The
sink plans it on commit (`plan_entry`). At emission
(`adaptive3d/path.rs`), `segments_to_toolpath` makes it a stay-down
lift-over link (`try_emit_stay_down_link`: Linking feeds up, across and
down to the rapid floor, then the entry style down) or a retract and
plunge. The `Rapid`s come from these places, and each one is a cause:

| Cause | Where (clearing.rs, `plan_and_emit_region`) |
|---|---|
| `Start` | the first 2D `Rapid` of an outline's plan (the plan's entry) |
| `Reentry2d` | every later 2D `Rapid` of the plan: a refusal of the replay. It also carries the C37 join class, by its XY, from that outline's `JoinsRecord` |
| `Link2d` | a 2D `Link` that is not held (a shipped arm, or a `Link` before any position) |
| `LargeZ` | the large-Z split of a draped cut: `dz > max(1.1·DPP, 1 mm)` or slope > 0.3 between two lifted points |
| `LargeZHeld` | the same split on a held link that C26b lifted as a cut (the `held` flag) |
| `AirRun` | the end of an air run longer than `air_run_crossover_mm` inside a cut |
| `Sweep` | the shipped perimeter sweep's entries (exterior and holes) |
| `Other` | an entry of the toolpath that no record matches (must stay near 0) |

F-038 makes no entry of its own: it removes groups, and the next group's
`Rapid` is `Reentry2d`. So it is not a cause in this list. The C22a
replay after the filter shows up as more `Reentry2d`.

### 15.2 The design

- **Record at the source.** At each `sink.emit(ctx, Adaptive3dSegment::Rapid(p))` site above, a research-only push of `(p, cause)` to a new `RefCell<Vec<EntryCause>>` on `ClearZLevelContext`.
  - It is rolled back with the pick and with C38's `RecordMark`, as the other research records are.
  - It changes no segment and no emitted move.
- **Join at the emission, by position.** In `research_turn_feeds` (adaptive3d/mod.rs), after `segments_to_toolpath`, walk the toolpath's moves:
  - An entry is a run of entry-intent moves.
  - Its kind is a **lift-over** when the move before the run is a `Linking` feed, or a **plunge entry** when it is a rapid.
  - Its end XY is the entry point.
  - Its drop is the Z where the run starts less its end Z. The fixture rule counts a descent at 1 mm or more.
  - The cause is the record whose XY equals the end XY, to 1e-6 mm. The emitter moves the entry point by none of its steps, so the key is exact. No record means `Other`.
- **One `info` line per entry** in CLI runs: `C40 entry kind= cause= join= x= y= z= drop_mm=`.
- **One `info` line per op** with the counts by kind × cause, and by cause for the drops of 1 mm or more.
- `Research3d.entries` holds the same records, for the C12 tests. The tests print them and assert only `Other` = 0 on the pocket.
- **What the instrument cannot see:** whether a descent removes stock. That is the simulation's (entries.py: `removed_volume_est_mm3`). D9 joins the two by XY. Each C40 line carries the exact entry XY, which is the sample XY at the end of the descent.
- No new constant: the causes are the existing branches, with their existing thresholds.

### 15.3 Pre-registration (P; D24 build, arm `geodesic_local_spiral_pick`; the descents of 1 mm or more that remove stock, after the join with entries.py)

- **Corne op 1, 1010 such descents in D23:**
  - `Reentry2d`: 140–200 (14–20 %). §13.9: the 2D re-entries are 161, and only 188 of the head-on arm's descents lay near one.
  - `LargeZ` + `LargeZHeld`: 550–780 (55–75 %). Most descents end off the level planes (591 of 1010 in D23 pick) on the part's top faces. That is the large-Z split's signature. `LargeZHeld` of those: 20–100.
  - `AirRun`: 80–220 (8–20 %).
  - `Start` + `Sweep`: 10–40. Op 1 has 4 levels and 1–3 regions each.
  - `Link2d`: 0 (every research link is held). `Other`: ≤ 1 %.
- **wanaka200, 2016 such descents in D23:**
  - `Reentry2d`: 140–170 (about 8 %). The 163 2D re-entries, 153 of them OverBound.
  - `LargeZ`: 1500–1850 (75–90 %). §12.8: the Z 8.62 hops are short (median 0.8–1.7 mm in XY) and drop about 4.3 mm, under the dz rule's 6.0 mm. So they are the SLOPE rule (`dz / xy > 0.3`) on the draped terrain.
  - `AirRun`: 50–250. `Start` + `Sweep`: ≤ 20. `Other`: ≤ 1 %.
- **Falsifiers:**
  - If `LargeZ` is under 50 % on either fixture, my reading of §12.8 and §13.9 is wrong.
  - If `Other` is over 1 %, the join by XY is not exact, and I report it before reading any split.
- **What the result decides:** if `LargeZ` dominates as predicted, the next item is the 3D stage's split rule, not the 2D joins. The candidates are a large-Z split that rides the drape at feed when the stock above it is cut, and a slope threshold from the tool and the ruled pass load in place of the fixed 0.3. That is a design for after C40, not part of it.

### 15.4 Built (2026-10-26; not compiled until D9 runs it)

- **mod.rs:** `EntryCause`, `EntryCauseRecord`, `EntryKind`,
  `EntryRecord` and `Research3d.entries`. `entry_records` holds the join
  by exact XY (`xy_key`, 1e-6 mm) and the run rule:
  - an entry is a run of entry-intent moves, with the peck's retracts at the same XY;
  - its kind comes from the last XY traverse before it: a feed is a lift-over, a rapid is a plunge.
  It logs one "C40 entry" line per entry, and one "C40 entry summary" line per op.
- **clearing.rs:** `record_entry_cause` before each `sink.emit(.. Rapid ..)`:
  - the sweep's 2 sites;
  - the cut branch: the air-run flush, the large-Z split (`LargeZHeld` when `held`) and the final air flush;
  - the 2D Rapid arm: `Start` when it is the first `Rapid` of its outline's plan, else `Reentry2d`;
  - the 2D Link arm.
  The pick rolls the records back.
- **No change to C12's `RecordMark`.** The G-b judge rolls back before
  the emission, so no cause record of the skipped region exists yet. The
  sweep's records come before the mark, as the sweep's entries do.
- **Tests:** the C12 plain and heavy tests print `entry_line`, and assert
  that no entry is Other.

### 14.5 D24: why three C22a predictions missed (2026-10-26)

Read only. `phase3/fix14/c22a/assert_14_5.py` asserts each number below
from the D24 artefacts. D24 holds C22a AND C12's removal of the corridor
opening, so I split the two where the data allow.

**1. Three rounds (P ≤ 2).** Corne has 17 of 40 replay lines at 3 rounds,
and the most groups that a later round dropped is 271 (copy op Z 2.01,
region 2, after 47 in the first filter). In at least 10 plans the later
rounds drop more groups than the first filter did.
- **The cause:** a cascade. The replay re-walks each cut under the pass load. A cut that stood on ground a dropped group had cleared is split there, and its short pieces become new groups under the threshold. The next filter drops them, which uncuts more ground under the cuts after them, and so on.
- The rounds equal the depth of that chain of dependencies, not one level as I assumed. The loop still ends (the bound is the group count), but my "≤ 2" was wrong.
- **The cost of the cascade:** each round drops cuts that removed real stock, only because the split made them short.
  - Inside the silhouette, where C12's corridor removal does not act, the op-1 lift-over descents moved 587 → 606 (+19). That is within my ±20.
  - So the cascade's harm to op 1 is small here. Its effect on the left stock is not measured apart from the corridor removal.

**2. The wanaka counter (66 lines; 33 in the kept plan; P 0).**
- Every flagged point lies OUTSIDE the emitted toolpath's XY box (22.76–217.24 × 27.76–221.74): x 219.3 / 20.8, and y 25.874 / 223.874.
- No move of the G-code goes past that box.
- 34 of the 66 read exactly the full stock above the level (25 − Z: 5.46, 10.92, 16.38). No cut was ever stamped there.
- **So the counter reads the stock correctly, but it judges the held link BEFORE the level's clip to the boundary.** The link's part outside the containment is not emitted and not stamped, so the sim sees no cut.
- The counter is wrong in where it reads, not in what it reads. It must test only the points of the link that are emitted, after the sink's clip. That is a counter fix, and the link itself is not a fault.

**3. The Corne fed-link bar (move 6921, 10.50 mm, counter 0).**
- Move 6921 is a `Linking` feed AT the level (all samples Z 11.00), 2.25 mm long, between `clearing_cut` moves. It is a held link (`HeldLinks`, keep-down), not a C26b lift (that one goes out as a cut) and not an F-038b stay-down link (that one runs above the level).
- The clearing cuts around it at the same place also remove 12.0 mm, which is the D24 deep-cut line.
- So the stock there is the containment band, beyond the mesh bbox, that the planner reads as AIR: G-CLIPAIR, C38 §10/§11. The counter reads the planner stock, so by construction it cannot see stock the planner believes cut.
- The link is the same fault as the cuts beside it. C12's G-CLIPAIR fix (research_clip) is the fix for both. The counter needs no change for this case.

**4. Not asked, but in D24's table: the Corne reversals (> 135° at move junctions)**, D23 pick → D24:
- inside the silhouette 36 → 40;
- outside 10 → 2217.
They are in the outer strip and beyond the bbox, where D23's corridor opening planned. So the 3007 planar turns, and most of the +473 s, come from the corridor removal (the agent's narrow loops across the strip), not from C22a. The 3–9 mm band (455 → 999) is in the same strip.

**What follows:**
- **The C22a counter:**
  - Read the link after the sink's clip, i.e. move the test to where the sink stamps the held link.
  - Report it separately from G-CLIPAIR. The counter cannot see stock that the planner reads as air.
- **The cascade:** the filter-and-replay loop is correct (the replay reads the plan that goes out), but it can erode a plan through a chain of splits. If D25 (with G-CLIPAIR) shows that the erosion leaves stock, the alternative is C38 §9.5 item 2 (no group filter in research regions). I would measure it before deciding: the C22a info line already gives `dropped_more` per plan.

### 14.6 wanaka200: 72 of 77 cut turns "unfed" (2026-10-26)

Read only. `phase3/fix15/c22_turns/assert_14_6.py` asserts each number.

**The record:**
- D24: planned 77, cut 74, fed 2, unfed 72, full_feed_unmatched 0, and all 74 with a factor under 1.
- D25: planned 331, cut 77, fed 5, unfed 72.
"Unfed" means that `apply_slice_turn_feeds` found NO emitted `Linear` move whose start and target both lie within 1e-6 mm of the turn's XY segment. It does not by itself mean that a move cut the turn at full feed.

**What the D24 simulation shows:**
- Every wanaka clearing sample runs at 1000 mm/min, except 4 samples on 2 moves (feeds 377 and 428): the 2 fed turns. They lie 34–41 mm inside the emitted toolpath's XY box (22.76–217.24 × 27.76–221.74).
- No cutting sample lies outside that box.
- The 2D plan DOES reach beyond it: all 66 D24 held-link counter points lie outside the box, at x 219.3 / 20.8 and y 25.874 / 223.874 (§14.5). The C31 grown outline extends past the containment line, and the boundary clip (`clip_segment`) removes what lies beyond it.

**The three candidates against this:**
- **The merge (C12 §10.1):** not on its own. `split_at_turns` (D14) makes each turn a `Cut` of its own, so the one RDP keeps its two ends. Corne, under the same code, feeds 132/132 and 230/230.
- **The drape:** it changes Z only. `drape_path_to_leave` densifies along the segment, and a large-Z split cuts it at a point on it, so the pieces' XY still lie on the turn's segment and still match.
- **The setup-0 flip:** the turn feeds are applied to the toolpath that `segments_to_toolpath` builds in the planning frame. The work-frame transform comes after. A frame error would leave 0 turns matched, not 2–5 near the middle.
- **What fits: the boundary clip.** On wanaka the open arcs are the block's corner arcs, 153 of the 164 re-entries OverBound (§13.9). They end at the 2D region's edge, which lies BEYOND the containment. So the turns that join their ends lie outside it too. The clip drops them, so no move exists to feed: they are not cut at all.
  - `SliceTurns::add` counts a turn as cut from the 2D plan (`turn_in_plan`), before the clip, so it reads "cut, unfed".
  - On Corne the turns are at inner walls, inside the containment, so they are fed.

**Since when:** reduced-feed clearing moves in the wanaka sims: D20 0, C35 1, D24 2 (Corne D24: 383). So this has been so since the turns came to the 3D arm. The summary line (D24) only made it visible. The grow past the containment dates from C31.

**The load:** if my reading is right, these 72 turns put NO load on the tool: they are clipped away. Their cost is elsewhere. Where the clip cuts an arc, the next piece gets a clip entry (the C40 `Clip` cause), that is, a descent.
- If my reading is wrong and they are cut at full feed, each one cuts `w = c / factor` of D: factor 0.365–1, so up to 2.7× the ceiling width, the 0.75–0.94 D turn cap of C14.
- The D24 sim has 324 samples over 0.471 within 3.2 mm (the turn bound) of the box edge, against 2142 farther in. They are not attributed to turns.

**Proof needed (no code yet):** one instrument line per unfed turn:
- its from / to;
- whether each end lies inside the centre clip (`CentreClip::contains`);
- the distance to the nearest emitted move segment.
P: about 70 of the 72 have an end outside the clip and no move within the turn bound. The D26 C40 summary on wanaka should also show `Clip` entries at Z 19.54 / 14.08 / 8.62, about one per clipped arc end.

**The fix, if it is confirmed:**
1. **The record:** count a turn as cut only when its segment survives the clip, so a clipped turn is "clipped", not "unfed". Give the summary a `clipped` count.
2. **The cause:** the research arm should not plan beyond the containment. Clip the C31 grown outline to the centre boundary as well as to the stock box (C38 item B), so that the 2D plan, its turns and its held links all lie where the 3D stage may emit them. This is C12's code (C31 call site / research_clip), so it is for C12 and the lead.
3. **C12's chain match (§13.2) is not the fix here.** It finds moves that a merge changed, but a clipped turn has no move at all.

**Built (2026-10-26, on top of D26; not compiled):**
- `SliceTurns.ends_inside`: one `(start inside, end inside)` per record. `SliceTurns::add` takes `inside`; clearing.rs passes `CentreClip::contains`, or "always inside" with no boundary.
- In `apply_slice_turn_feeds`, a record with no move found and neither end inside is `clipped`: it is counted in `TurnSummary::clipped` and left out of `turns` and `unfed`. A clipped turn whose middle crosses into the clip still gets its feed, because it stays in the match.
- One "C22 turn not fed" line per turn that is not fed, with `why` (clipped / unfed / full feed, no move):
  - from / to;
  - from_inside / to_inside;
  - the factor;
  - the nearest move's distances (`HoldingMove`).
- The summary line gains `clipped=`.
- No move changes. With no boundary (the C12 pocket) every end is inside, so the record is as before.
- **Open item (agreed with C12):** in a later build, drop `kind=slots`
  from the "C22 turn summary" line. The exact match reads C12's merged
  slot steps as unfed. Until then, a reader takes slot counts from the
  "C38 slot chains" line only.

### 12.9 C35 regression: three Corne hole floors left uncut (2026-10-26)

Read only. `phase3/fix17/c22_holes/assert_12_9.py` asserts the numbers.

**What the simulations show.** Below are the op-1 cutting samples within 1.5 mm of each hole centre, below Z 1.0 (the Z 0.5 level). The seed is (55.87, −49.76).

| Hole | r from seed | D20 | C35 | D24 |
|---|---|---|---|---|
| (28.3, −30.0) | 33.9 | 11 cut, 2 entry, 4 link | 0 | 0 |
| (80.3, −40.0) | 26.3 | 6 cut, 4 entry, 4 link | 0 | 0 |
| (45.6, −80.1) | 32.0 | 9 cut, 4 entry, 4 link | 0 | 0 |

- Both arms cut these holes at Z 17, 11, 5 and down to the drape at 4.0.
- Only the Z 0.5 level is missing in C35. D20 went into each hole at Z 0.5 with an entry (a descent from 5.5 to 0.5), so its plan reached each hole as a separate piece of a level set: a small closed wrap, joined by a re-entry.
- At Z 0.5 the C35 core of region 2 ends at r 33.975 (the C35b hand-off line, 33.975 → 35.175). **All three holes lie inside that core disc** (26.3, 32.0, 33.9).

**The mechanism (from the code; the field is not in the artefacts, so this is a reading, not a measurement):**
- `held_core_spiral` takes K from `star_levels`. That test asks only that each ray from the seed crosses each level set ONCE before the ray first leaves the reachable cells. Pairs of samples with an unreachable one between them do not count (`ray_crosses_once`).
- A hole pocket that the plate cuts off along the ray is behind unreachable cells on that ray. If its own T values (reached by another way) are all on one side of the level, it adds no crossing. So the level "is star-shaped" even though part of its level set is a separate small loop in the pocket.
- The spiral traces only the first crossing on each ray: the main loop. Then `ordered_wraps(.., spiral_levels + 1, ..)` skips EVERY wrap of level ≤ K, the pocket loops included.
- So the core claims the pocket's levels and never cuts them. That is the lead's third hypothesis. At Z 17, 11 and 5 the holes are open to the main area (the plate top is below those levels or the pocket joins the main zone), so there the spiral or the outer wraps cut them.
- The other holes inside the disc are cut at Z 0.5. I expect their pockets to have their T beyond T_K (their loops are kept as wraps of level > K) or to be joined to the main zone. These three sit near the core's edge (26–34 against 33.975), where a pocket's T can fall under T_K. That is a prediction for the instrument, not a measurement.

**The fix (for approval; research arm only; no new constant):**
- `ordered_wraps` keeps the wraps of levels ≤ K that the spiral does not trace. It drops only the level-k piece that is the closed loop round the seed (the one whose ray crossings the spiral follows). Every other piece of a level ≤ K (a pocket loop, an open arc) stays in the plan as a wrap. The test: a closed wrap of level k ≤ K that contains the seed is the core's; anything else is kept.
- **Instrument first, in the same build:** the core-zone record gains the count and the area of the level ≤ K pieces that are not the core loop, and whether each one was planned. P: at Corne Z 0.5 region 2, three pockets (the three holes, about 50 mm² of floor each). At Z 17, 11 and 5: none or few. On wanaka: none (the block has no pockets inside the core).
- **The generic point (operator ruling 2026-10-26):** "a star-shaped core claims only the area its spiral sweeps" is a rule of the core spiral itself, not of this arm. It belongs where `held_core_spiral` lives, and any strategy that adopts a core spiral gets it.

**Pre-registration (P; D27 + this fix, Corne, spiral_pick):**
- The three hole floors read 0.00 above target (D20's value).
- +3 entries at Z 0.5 (one per pocket), and about +30 s of cycle.
- No other stock-map cell changes by more than 0.1 mm.

**Built (2026-10-26, for D28; not compiled):**
- `SpiralCore { seed, levels }` and `SpiralCore::owns` (a closed chain of a level ≤ K that encloses the seed). `beside_core` drops only those.
- `ordered_wraps_beside_core(.., core, ..)`: with a core, the chains start at level 1 and only the core's own loops are dropped. `ordered_wraps` is now a wrapper with no core, so every other caller is unchanged. The C35 call in `plan_piece` passes the core.
- The record:
  - `FieldReport.core_pieces` lists each kept wrap of a core level (level, closed, area);
  - `CoreZone.pieces` and `CoreZoneRecord.core_pieces` / `core_pieces_closed` / `core_pieces_area_mm2` carry it;
  - one info line per core: "C35 core: wraps of a core level beside the core loop".
- The lib test `only_the_core_loop_leaves_the_wraps_beside_a_core_spiral`: the core loop goes; a pocket loop of a core level, an open arc of a core level and a loop past K stay.
- **A doubt I record before D28 reads it.** If T is a true distance (T·s at least the straight distance from the rim), a pocket that the plate cuts off along a ray lies beyond the point where that ray leaves the reachable cells. Every level ≤ K crosses the ray before that point, so the pocket's T exceeds T_K, its loops are of a level > K, and they were never dropped.
  - So the mechanism of §12.9 needs T under the straight distance in the pocket. The lattice solve could do that, or the pocket could be joined to the main zone behind the plate on another path.
  - Or the cause is elsewhere: the pocket reached by no field at all, with the mop or the wall pass not reaching it in C35 but reaching it in D20.
  - The D28 core-pieces line and the hole floors decide. If the floors stay uncut and the core-pieces line shows no pocket at Z 0.5, my mechanism is wrong, and the next step is to log the unreachable lattice cells inside the region per core.

**Built for D27 and RULED (lead, 2026-10-26): the held-link keep-down proof (C38 §14).**
- `clearing::keep_down_excess(stock, from, to, radius, ceiling_z)` reads the swept disc of a link as `corridor_ceiling_z` does. It is generic: any strategy's held links may call it.
- A research held link stays keep-down only if no column of its R disc stands above `level + Depth/Pass + stay_down_clearance` (the top of its level's slab plus the dexel allowance). Otherwise it goes out as an entry (C40 cause `HeldLinkRefused`).
- The C22a counter reads the same disc and ceiling, after the clip.
- **The ruling:** the ceiling is the SLAB TOP, not `level + clearance`. A link held to the 2D pass load may take a side band up to `c·D` inside its own slab (G-ADAPTLINKLOAD). The fed-link bars are stock ABOVE the slab ("removes more than Depth/Pass + one cell"). Accepted by the lead.

## 16. Design: the 3D split rule and a shared routed keep-down travel (2026-10-26; read only, no code)

Inputs:
- C40 (D26, fix16): LargeZ makes 74.2 % of Corne op-1 descents and 90.0 % of wanaka's.
- C21's C42 (fix18/README.md): the spiral_pick descents are SIDE STEPS. Lift and plunge are almost at one XY (median 0.57 mm Corne, 1.22 mm wanaka) and the drop is 5.16 / 4.16 mm.
  - 72.6 % / 69.1 % have a path over cut ground plus a short side leg (median 0.58 / 1.19 mm).
  - The side leg is within a stepover and its depth within Depth/Pass for 64.2 % / 55.4 %.
  - Taken at the cut feed, these save 380 s of 1452 s (Corne op 1) and 1313 s of 6026 s (wanaka, sample clock).
  - contour_parallel's entries are long hops (median 37 / 64 mm), where a keep-down at feed pays only on the short ones.
- The operator ruling: the keep-down link is a shared part, not research-arm code.

### 16.1 What makes the side steps

`plan_and_emit_region` splits a draped cut where two lifted points step by `|dz| > max(1.1·DPP, 1 mm)` or by a slope over 0.3 (`PLUNGE_SLOPE_LIMIT`, `Z_DROP_FLOOR_MM`, clearing.rs). Its comment gives the purpose: "peak→valley bridges that drag the cutter diagonally through uncut stock".
- **The source of the numbers:** none. 0.3, 1.1 and 1 mm are typed constants, and the 1 mm floor was tuned against over-splitting. They are not derived.
- **What they test:** they test the PATH (its slope), not what the path cuts. On a draped level the path climbs onto a part face and drops back many times. Each such step splits the cut, lifts and re-enters about 1 mm away (C42). In most cases the step cuts no more than an ordinary pass would: the 64 % / 55 % above.

### 16.2 Part A: the split rule reads the stock, not the slope (all adaptive3d strategies)

**The rule (derived; it replaces the three constants):** split a draped cut between two lifted points `p → q` only when feeding it would take more than one pass, that is, when the tool's swept disc along `p → q` meets stock deeper than Depth/Pass below its top.
- **The read:** for each sample `x` along `p → q` at the planner cell (as `corridor_ceiling_z` samples), with the tool bottom `z(x)` on the segment, the stock top over the disc of R at `x` is `top(x)` (the conservative read).
- **The axial engagement:** `ap(x) = top(x) − z(x)`.
- **The split:** where `max ap(x) > DPP + stay_down_clearance`. Otherwise `p → q` is cut at feed as part of the cut. That is the planner's own pass rule (F-029 clamps each cell's cut to Depth/Pass).
- **Its sense:** a descent through air (`top ≤ z`) never splits. A climb onto a face whose remaining stock is within a pass is a cut. A bridge "through uncut stock" (the stated purpose) has `ap > DPP` and still splits.
- **Where it lives:** in the 3D emission of `plan_and_emit_region`, the one place every adaptive3d strategy (AgentSearch, contour_parallel, geodesic) drapes and splits a cut. It reads the sink's planner stock, which already holds every earlier move. A helper `cut_step_within_pass(stock, p, q, R, DPP, clearance) -> bool` sits beside `keep_down_excess` (the same disc read).
- **Shipped arms:** byte parity forbids changing them in the same step. The rule goes behind a research switch first, keyed as `disc_read` is today (`is_research_arm`). Shipped adoption is a separate, parity-checked step, as ruled.

### 16.3 Part B: a routed keep-down travel at the entry layer (shared)

Where Part A still splits (`ap > DPP`), and for every other entry (2D re-entries, clip entries, waterline):
- **The component:** `keep_down_route(stock, from, to, R, level_z, clearance, max_len) -> Option<Route>` in a new module `adaptive3d/keep_down_route.rs`.
  - **The search:** a shortest path over the planner grid's free centre cells (a cell is free when `keep_down_excess` over its disc of R is `None` against `level_z + clearance`, the slab-top ruling of §14 for a held link). The search runs from the tool position to the free cell nearest `to`.
  - **The side leg:** from there to `to`, accepted only as a cut within a pass (Part A's rule) and within one stepover (the side band a pass may take). C42's method: 16 neighbours, a window of the two points plus a margin.
  - **The result:** the route as XY points at the level and the side leg, or `None`.
- **The call site:** `plan_entry` (clearing.rs). It is the one door every adaptive3d entry passes, so every strategy gets it. Today it tries one straight stay-down corridor (`corridor_ceiling_z`). With Part B it tries the straight corridor first, then the route.
  - The route is emitted as keep-down `Linking` feeds at the level, then the side leg as a cut. Its proof is the same disc read.
  - The emitter needs one new segment, `Adaptive3dSegment::RoutedLink { path, side_leg }`, or the route as a `Link` chain plus a `Cut`. The second needs no new type and is my preference.
- **Feed or lift:** the route is taken only when it is faster than the entry it replaces. Travel at the link feed costs `len / feed`; the entry costs the retract, the rapid and the plunge feed. That is the existing crossover rule `air_run_crossover_mm(feed, plunge, retract_depth)`: no new constant.
  - That decides contour_parallel's long hops (C42: at feed they rarely pay) without a strategy switch.
  - A low travel at the RAPID rate (C42's larger saving on contour_parallel) is a separate question: a rapid at depth over cut ground. It needs its own safety ruling; I do not include it.
- **Bounds:**
  - The search is bounded by `max_len = air_run_crossover_mm(..)`: a longer route can never pay. So the window and the cost are bounded, with no typed length.
  - The grid is the planner stock's (Corne 0.25 mm, wanaka 1 mm). The work per entry is at most the window's cells.
- **Who else can use it:**
  - every adaptive3d strategy, through `plan_entry`;
  - the 2D adaptive's keep-down links (`feed_link_to_legal`) could call the same search on the 2D grid, but that is a separate adoption step;
  - the finishing ops' link layer (`dressup`) has its own link rules and is not in scope.

### 16.4 Order and pre-registration (P)

**Order:** Part A first (a rule change, research switch), then Part B. Part A removes most side steps at their cause: they are not entries at all. Part B then serves what remains.

- **P after Part A (spiral_pick; base D27 when it reports):**
  - Corne op-1 descents (≥ 1 mm, removing stock): 1156 → **about 450–550**. C42's 64 % within a pass, read on the swept disc rather than C42's centre cell, so a little fewer qualify.
  - wanaka: 2030 → **about 900–1100** (55 %).
  - Cycle: Corne about −250 to −380 s; wanaka about −900 to −1300 s. C42's cut-feed savings, less the cut time the steps now take.
  - LargeZ's share falls below Reentry2d's on Corne.
  - The over-load samples (0.471) on the draped steps must not rise by more than 5 %. A step within a pass is an ordinary cut, but its swept width is not held to the 2D ceiling, so this is the risk to watch.
- **P after Part B:**
  - a further −10 to −20 % of the remaining entries on spiral_pick;
  - on contour_parallel (once adopted), routes on at most about 30 of Corne's 95 entries and about 450 of wanaka's 1609. C42: the routes faster at the cut feed are 28 and 449.
- **Falsifiers:**
  - Part A removes under 30 % of Corne's descents: the steps are deeper than one pass, and Part B, not A, is the main fix.
  - The over-load on the steps rises by more than 5 %: the rule needs a width read too.

### 16.5 What I need before code

1. C21's measurement of the side legs on the swept disc (C42 used the centre cell), to size Part A's share.
2. A ruling on the research switch for Part A: `is_research_arm` (as `disc_read`), or a new `Adaptive3dParams` field off by default.
3. Whether Part B may add `Linking` feeds at the level for a shipped arm later. Today's F-038b link climbs to a safe Z over the corridor; a level link is a new shape, and the simulation's fed-link bar reads it.

### 16.6 Built (2026-10-26; not compiled)

**Part A:**
- `Adaptive3dLinking.step_split: StepSplit { PathShape (default), StockDepth }`. The research arm forces `StockDepth` in the ctx.
- `clearing::step_axial_depth(stock, p, q, R)`.
- In the split loop, a step that the path rule flags is split only when `ap > DPP + stay_down_clearance`.
- It is evaluated only on flagged steps, for cost: so it removes splits and never adds one.
- Instrument: `SplitStats { made, avoided, max_ap_avoided_mm }`, and the line "C22 step split (stock-read rule)".

**Part B:**
- `Adaptive3dLinking.keep_down_route: KeepDownRoute { Off (default), CutFeed, Rapid { clearance_mm } }`.
- New module `adaptive3d/keep_down_route.rs`:
  - an A* over free planner cells (the full disc of R at most `ceiling_z`, and inside the boundary clip);
  - sources: the free cells within two cells of the tool;
  - goal: the first free cell within a stepover of the entry whose side leg passes Part A's rule;
  - length bounded by `stay_down_mm`;
  - `straight_runs` merges the grid's straight runs.
- `clearing::routed_entry` builds the replacement:
  - cut feed: `Link`s at the entry's Z, free to the stay-down clearance;
  - rapid: the new segment `Adaptive3dSegment::LowRapid` at Z + clearance, free to that clearance (the full-disc proof);
  - a tool below the travel rises at its XY first; a tool above it drops only where its disc is free;
  - the side leg goes out as a `Cut`.
- Hooks:
  - `push_segment_with_stamp_unclipped`: every non-deferred entry of every adaptive3d strategy;
  - `LevelSink::commit_pending`: the deferred entries, spliced in place.
- `LowRapid` is handled at all seven matches: the tally, the emitter (a G0 tagged `Linking`), the clip, the stamp (none), the cursor, the sink and the C29 score.
- `StampDrape` carries the route setting, the clearance, DPP and the stepover. It is `Off` for the waterline.
- The research arm gets `CutFeed` when the parameter is `Off`. `RS_CAM_RESEARCH_KEEP_DOWN_ROUTE = off | cut_feed | rapid:<mm>` flips it for D9. The rapid clearance comes from the run's setting; no number is typed in the code. The derivation goes here when C21 reports the fuzz.
- Instrument: `RouteStats { tried, taken, travel_mm, side_mm }`, and the line "C22 keep-down route".

**Risk to watch:** a side leg and a step cut at feed are not held to the 2D width ceiling. The heavy C12 test's exact-bound check is the gate.

### 16.7 D29 v2 heavy RED: the exact bound broken by Part B's side legs (2026-10-26; read only)

From `phase3/d29/v2_10_c12_heavy.txt`. Every first over-sample the lead names sits right after a ROUTE, that is, a `Linking` feed at the level where an entry stood before:

| Mode | The moves before the over move |
|---|---|
| curv / oneway / zigzag / v2 / pick (move 64, exact 0.86) | 63 `Linking` 0.556 mm back to the free cell (−16.0, −9.0), then 64 the `ClearingCut` side leg, 1.984 mm (≤ the stepover 2) forward into stock to (−14.02, −8.88) |
| spiral_pick (move 285, exact 0.98) | 283 `Linking` 0.890 mm, 284 a side leg of 1.298 mm, then 285 the next planned cut, 1.79 mm |
| headon (move 2942, exact 0.82) | 2930–2941: a 28 mm `Linking` route, then 2942 a side leg of 1.905 mm |
| zigzag (move 644, exact 0.875) | 642 `Linking` 0.587 mm, 643 a side leg of 0.940 mm, then 644 the next planned cut, 2.06 mm |

**The cause (B, not A):**
- The entry B replaces was a re-entry INTO stock: a plunge (or helix) that cut the column and left a hole of the tool's own size. The 2D replay stamped that hole (`LinkLoad::stamp_entry`) and exempted the steps out of it (the plunge-departure exemption, G-ADAPTPASSLOAD: "leaving a hole of the cutter's own size reads at least half the diameter whatever the heading").
- B replaces the vertical entry by a HORIZONTAL side leg into the same stock. My side-leg rule held its length (≤ stepover) and its depth (Part A's ap ≤ DPP + clearance), but not its WIDTH. A side leg into a column is a slot: 0.86–0.98 D.
- The planned cut after it then leaves a "hole" that is not a hole, so its first steps meet the stock the plunge would have taken (moves 285 and 644). They are no longer departures from an entry, so the heavy test judges them, rightly.
- So B as built turns an exempt plunge into an unexempted full-width side cut at the full feed. The over is real load, not a reading artefact.

**What this means for B:**
- The entries B can replace without breaking the load are the ones that cut nothing: the entry disc is already free (the lift-over descends through air). There the route replaces only the lift and the traverse, with no side leg.
- For an entry INTO stock, a horizontal approach can hold the load only if its swept width stays within `c·D`, and the cut after it then still lacks the plunge hole. Both are needed, and the second is not a local property.
- C42's side steps are entries into stock by its selection. So B can save little of C42's 380 s / 1313 s safely. The saving is A's: steps that are no entries at all.

**Proposal (no code until the ROUTE=off result separates A):**
1. B: route only to an entry whose own disc is free at its Z plus the clearance (`free_at(entry)`), with the side leg removed (the route ends on the entry cell). Count the refused ones in `RouteStats`.
2. A: kept, if the ROUTE=off heavy run holds the exact bound. If it does not, A's steps need the same width read, and I report it with that run's probes.
3. A generic note for the shared router (the operator ruling): "a keep-down travel replaces the travel, never the entry's cut". It belongs in `keep_down_route.rs`'s header.

### 16.8 The B fix and the Part A gate (2026-10-26; clippy clean, not tested)

- **The ROUTE=off heavy run is identical to D27 v3**, so every D29 over was B. Part A WAS on in that run: `path.rs` sets the ctx's `step_split` to `StockDepth` for the research arm whatever the test's parameter says. But the two-level fixture has no large-Z step, so A acted on nothing there.
- **The B fix (§16.7):**
  - `routed_entry` refuses an entry whose own footprint is not free (the entry floor disc, `plan_entry`'s read, against Z + clearance) and counts it as `RouteStats::refused_into_stock`.
  - The route ends ON the entry: a last `Link`/`LowRapid` to the entry point, and for the rapid a vertical `Cut` down the clearance (a link never feeds down). There is no side leg.
  - The header of `keep_down_route.rs` states the rule: "a keep-down travel replaces the travel, never the entry's cut".
- **The parameter** is now `keep_down_route: Option<KeepDownRoute>`:
  - `None` is the strategy's default (the research arm `CutFeed`, every other strategy off);
  - `Some(Off)` turns it off for the research arm too;
  - the env override still wins in research runs.
  - The literals already passed `Default::default()`, so none changed.
- **The Part A gate:** the new heavy test `the_stock_read_split_holds_the_exact_bound_on_draped_steps`.
  - Setup: one level from the top to the floor (drapes over the island and gives the large-Z steps), six G1 arms, `StockDepth`, route `Some(Off)`.
  - It asserts 0 judged samples over the exact bound per arm, the router not run, and at least one step cut at feed by the rule over all arms.
  - No existing assert is changed.

### 16.9 D29 v4 fixtures: Part A's over-load, the Corne rapid collision, and the turns (2026-10-26; read only)

Scripts are in `phase3/fix20/c22/`: `parta_place.py`, `turns_inside.py`, `turns_xy.py`, and `assert_16_9.py`, which asserts the numbers. D29 also holds C12's corridor opening and slot fix, so I split by place where the data allow.

**1. The over-load is on Part A's steps.**
- The D26 op-1 large-Z entries (the C40 lines with cause LargeZ / LargeZHeld: 874 on Corne, 2028 on wanaka) mark the steps that Part A now cuts at feed.
- Clearing length over 0.471, D26 → D29:
  - Corne: +496.9 mm in all; +426.7 mm (86 %) within R + one sim cell of such a step;
  - wanaka: +1139.8 mm in all; +1065.8 mm (94 %) there.
- So the rise is A's steps, and the cut around them, at the full feed. Part A held the axial depth of a step (ap ≤ DPP + clearance; max ap 6.0 on Corne, 5.46 on wanaka, which is a full Depth/Pass) and not its width. That is my §16 falsifier.
- **The proposed width read (no code yet):**
  - At an avoided step, also read the step's swept width on the planner stock. Take the cells inside the disc of R along the step whose stock stands above the tool's Z there plus the clearance. Their extent across the step's XY heading (the rule of the 2D `swept_width_on`, read in 3D) must stay within the pass ceiling `c·D` of the 2D load (`PassLoad::swept_width(s, R, cell).ceiling` × D, no new number).
  - If it is wider, the step splits as before.
  - It sits beside `step_axial_depth` and is generic.
  - Expected: the over-load falls back toward D26, and some of the descents come back. The P waits for the build.

**2. The Corne rapid collisions:**
- (91.3, −1.1): Z 28 down to 17.95 / 17.62 / 17.57. The three are the C40 entries `Plunge, Start` at (91.298 / 91.318, −1.128) at Z 11, 5 and 0.5: the first `Rapid` of an outline plan at each level. They follow C12's corridor openings in the log order, so they are corridor entries.
- They are NOT A's or B's entries (the router took 0 on Corne).
- But near that XY, D29's Z-17 cut now RIDES the frame top: clearing samples at Z 17.3–18.5. These are A's avoided steps. D26 lifted over the same place (linking samples at Z 22.7–23.5; the C40 LargeZ entries at (89.2–93.3, −1.1 to −3.6), Z 17.1–18.4).
- So the collision appeared where A cuts at feed over the frame top, and the planner's rapid floor there (17.95, then lower at each level) is under the simulated stock.
- That is a planner/sim divergence over A's draped steps, or C12's corridor entry reading the band as air. I cannot tell which from the artefacts.
- **Next:** a probe line at each entry: the planner's conservative disc top against the stamped path's top over the same disc. Or D9 reads the sim stock top at (91.3, −1.13) after the Z-17 level, for D26 and D29.

**3. The inside turns doubled because of slope, not heading:**
- `fix20/turn_place.py` uses `fix11/audit.py`'s 3D tangents, so a change of SLOPE counts as a turn.
- Inside the silhouette, D26 → D29: 3D 831 → 1767, but XY only 452 → 446.
- The +936 are the draped steps A now cuts at feed (up onto a face, then level again). Before, a split broke the chain with a G0.
- The heading of the path did not change. Whether a slope change at feed matters for the machine is the operator's call. The accel model, not the turn count, decides its cost.

### 16.10 Built: Part A's width read and the start-entry probe; pre-registration (2026-10-26; clippy clean, not tested)

**Built:**
- `clearing::step_swept_width(stock, p, q, R, clearance)`:
  - at each half-cell sample along the move, the grid cells inside the disc of R whose stock top stands above the tool's Z plus the clearance, less the cells the move has already swept (the capsule from `p`);
  - their spread across the move's XY heading; the widest over the samples;
  - 0 for a move with no XY length.
  - It is the 2D `swept_width_on` rule read on the 3D planner stock, generic, beside `step_axial_depth`.
- **In the split loop (StockDepth):** a step within one pass deep is cut at feed only if its swept width is at most `c·D`. `c·D = radial_woc_fraction_from_leading_arc(pass_engagement_limit(s, R)) × 2R`, the formula of `PassLoad::swept_width`'s ceiling, so no new number. Otherwise it splits.
  - `SplitStats.width_split` counts these splits. They are inside `made`, and the "C22 step split" line prints them.
- **The probe:** one "C22 start entry probe" info line per Start entry. It gives the entry XY and Z, `floor_top` (the conservative top over the entry floor disc, the value `plan_entry` reads for the rapid floor) and `centre_top` (the cell-centre top over the tool's disc). D30 reads it at (91.3, −1.13) against the simulated stock there.
- **The draped-step heavy test** also asserts that the width read split at least one step over all arms (`width_total > 0`), beside the 0-over and `avoided > 0` asserts. No assert is weakened.

**P (D30, spiral_pick, route as in D29; base D26 and D29 v4):**
- **Over 0.471:**
  - Corne 1277.7 → **at most 820 mm** (D26 780.8 + 5 %);
  - wanaka 3365.1 → **at most 2337 mm** (D26 2225.4 + 5 %).
  - Thick over within +5 % of D26 (399.1 / 463.3).
- **Descents (entries.py), some of A's steps coming back as splits:**
  - Corne 157 → **about 350–700** (D26 1112);
  - wanaka 349 → **about 700–1400** (D26 2016).
  - Low confidence. The share of A's steps that are wider than `c·D` is not measured. The 86 % / 94 % of the new over-load near the steps says many are.
- **Cycle:** Corne 2458 → **about 2600–2950 s** (D26 3247); wanaka 1991 → **about 2350–2950 s** (D26 3513).
- **Falsifiers:**
  - the over-load stays above +5 % of D26: the width read misses part of the load (the stock beside the drape, or the cut after the step). I report with the probes;
  - the descents return above 90 % of D26: the steps are mostly wider than `c·D`, and A buys little.
- **The probe at (91.3, −1.13):** if `floor_top` reads under the simulated stock top (≥ 17.95 + the rapid-floor buffer), the planner stock there disagrees with the sim, and the cause lies in the stamp of the draped cut. If `floor_top` reads higher than the rapid ended, the emitter's descent floor and `plan_entry` disagree.

### 16.11 The shelf fixture, the width read's descent bug, and G-FACESLAB (2026-10-27; tests green here)

**Ruling (a)** is implemented in `the_width_read_splits_a_wide_draped_step`:
- width splits > 0 over the arms;
- per arm, no more MOVES over the exact bound under the stock-read split than under the path-shape base.
Moves, not samples: one long move over one place reads 4 samples where two short moves over the same place read 2. That was the "extra" count. The test prints both runs' over lines.

**The 3 "extra" overs, read with the base printed:**
- The five shelf-side overs (y ≈ 15.8, exact 0.69–0.72) are the SAME in both runs: the same XY and the same exact peaks, base moves 658/708/754/799/819. They are not A.
- The island-side over at (8.1–8.4, 2.7–3.6) is one place in both. Base: moves 934 + 935, 2 samples, exact 0.994. A: one move 888, 4 samples, 0.988.
- One real extra place: move 866 at (8.35, −2.98), exact 0.999. A had cut the descent 864–865 (Z 7 → 4.5 beside the island) at feed, so 866, the cut after it, became a judged move. Under the split it was a departure from a plunge, which the test exempts.
- The cause of that one was a **bug in my width read**: it dropped a cell as "already swept" by its XY distance to the move so far, whatever the Z. A descent sweeps nothing below its own path, so the floor stock it descends into was not counted.
  - **Fixed:** a cell counts at most at the lowest Z at which an earlier sample of the move passed within R of it.
  - Result: the width splits rose (oneway 19 → 37, spiral_pick 36 → 69), and spiral_pick's over places fell to the base's: 6 moves against 7.
- **Test results:** both heavy tests are green through the lane (51.5 s): the draped-step test (0 over on six arms) and the shelf test. Both clippy commands are clean.

**G-FACESLAB (the shared overs, a pre-existing 2D/3D mismatch at faces):** the DEFECTS row draft went to the lead.

### 16.12 D30: the wanaka near-A over-load, and the Corne rapid collision (2026-10-27; read only)

Script: `phase3/fix21/c22/nearA_kind.py`; the assert script is `phase3/fix21/c22/assert_c22_16_12.py`. It splits `load_split.py`'s "near A's steps" length by the move each over sample lies on:
- 'on a step': the move climbs or drops by the path rule A replaced;
- 'after a step': a level move right after such a step;
- 'level': any other move.

**1. The wanaka near-A over-load, D26 → D30 (mm over 0.471):**

| | on a step | after a step | level | total |
|---|---|---|---|---|
| wanaka D26 | 114.9 | 79.2 | 1340.6 | 1534.7 |
| wanaka D30 | 153.8 | 258.3 | 1628.6 | 2040.7 |
| Corne D26 | 14.9 | 74.0 | 548.6 | 637.5 |
| Corne D30 | 21.5 | 82.3 | 459.1 | 562.9 |

- The steps themselves add little (+39 mm on wanaka): the width read holds them.
- The rise is the cut AFTER a step (+179) and the level passes near the steps (+288).
- **D30 ran BEFORE the §16.11 descent fix.** The width read then dropped a cell as swept by XY alone, so a descent's own floor stock did not count. That is exactly the "after a step" load: a descent cut at feed into stock that the next level move then meets (the shelf test's move 866). wanaka's terrain at Z 8.62 is mostly descents and climbs.
- The 'level' share was already 1340.6 mm in D26: the G-FACESLAB mismatch on terrain (the 2D plan reads the terrain faces as stock in its slab, and the drape lifts the cut). A does not cause that part.
- **P for D31 (the descent fix):** wanaka 'after a step' falls back toward D26 (about 80–130 mm) and the near-A total toward 1600–1800 mm. The remaining excess over D26 + 5 % is G-FACESLAB's level share plus the slots. If 'after a step' stays above 200 mm, the width read still misses load after a descent, and I report it with the probes.

**2. The Corne rapid collision at (91.3, −1.13):**
- The probe gives floor_top 17.451 / 17.119 / 17.073 at Z 11 / 5 / 0.5. The rapid ends at floor_top + 0.5.
- Near the entry, the Z-17 level is NOT one draped cut. It is a run of width-split micro-steps (sim moves 1930–1949): five lift-overs within 3 mm.
  - Each one plunges from 23.5 to the frame-top drape (17.62, 17.73, 17.72, 18.02, 18.57 at 2.1, 1.6, 1.4, 2.2 and 2.7 mm from the entry) and cuts a zero- or sub-millimetre move.
  - Then 1950 cuts on at Z 18.45–18.50 (2.7 mm away, inside the tool disc of the Z-11 entry).
- So the simulated stock inside the entry's disc stands to about 18.0–18.5 (the frame top's drape), but the planner reads at most 17.45 there.
- 17.45 is the part surface (16.95) plus the leave. That is `lift_to_level`'s surface-heightmap height, not the drop-cutter drape the emitter and the stamp use (`drape_point`, `drape_path_to_leave`).
- **My reading, not proven:** somewhere at this place the planner stamps at the surface-heightmap height and the emitter cuts at the higher tool-aware drape. Then the planner believes cut what the tool never reached. Two candidates:
  - a piece of the Z-17 path that the sink stamps but the emitter does not emit as cut, for example a zero-length `Cut` (moves 1934, 1938 and 1946 have no XY length);
  - the heightmap lift of a point that the stamp's drape does not raise, for example a single-point piece.
- **The probe I would add (needs code):** at the Start entry, the planner cell with the highest top inside the disc, its XY, and the Z-17 stamps that covered that cell. D31 then names the stamp.
- **Separately, the micro-steps are poor output:** five lift-overs in 3 mm. The width read splits them. A step that splits by width there is a climb onto the frame top with stock beside it. After the fix, these may become fewer or more; D31 shows which.

### 16.13 D32 probes: the planner cell under a Start entry, and the short cut pieces (2026-10-27; built, not run)

The lead ruled to build the cell-level probe of §16.12 for D32. Research only; the research arm only (`StampLog::on`).
- `StampLog` (`adaptive3d/clearing.rs`, on `ClearZLevelContext`) records every stamp of the run as `stamp_emitted_segment` stamps it:
  - kind `cut`: the fed polyline;
  - kind `entry`: the descent from its start to the draped entry;
  - kind `link`: the draped link.
  The waterline stamps go in too (`waterline_cleanup` now takes the context).
- **"C22 start cell probe"**, one per Start entry: the cell `plan_entry`'s rapid floor reads (`max_conservative_top_cell_in_disc`, the same walk as `max_conservative_top_z_in_disc`), its XY, its conservative top and its centre top, and the number of logged stamps that reached its centre.
- **"C22 start cell stamp"**, one per such stamp, lowest first: the stamp's level, kind, `cut_to` (the lowest tip Z plus the profile height at the cell centre, sampled at half a cell), and the tool position there.
- **"C22 short cut probe"**, at the sink (`LevelSink::emit_clipped`): each cut piece with fewer than two points or an XY length within the RDP tolerance (`cut_tolerance`), and whether the sink stamps it (two points or more).
- The lines wait in the log until the C29 region pick keeps its variant. A scratch run's records leave the log after it runs and only the kept run's come back. So the B run never reads A's stamps, and a variant that is not kept prints nothing. The level start and the run end print them.
- **Caution:** the older "C22 start entry probe" line prints at once, so it also prints for the variant the pick does not keep. Read its D30 values with that in mind.

**P for D32 at Corne (91.3, −1.13), Z 11:** the highest cell sits within the entry floor disc at top ≈ 17.45. Its lowest-cut stamp is from level 17, and its `cut_to` is near 17.45, below the sim's 18.0–18.5. The kind of that stamp names the cause:
- `cut` with a zero XY length, together with a "C22 short cut probe" line at the same XY, confirms the zero-length candidate;
- `entry`: the entry stamp descends below the emitted plunge;
- `link`: a link stamp the emitter does not feed.
If every `cut_to` there is at or above 18.0, the planner never cut the cell to 17.45: the read is then the conservative bound itself, and the collision cause lies in the sim/planner cell mismatch, not in a stamp.

### 16.14 D31: the wanaka over-load rise is not on the steps A keeps; it sits where D26 plunged (2026-10-27; read only)

Scripts in `phase3/fix22/c22/` (the D31 run directory):
- `d31_rise.py` writes `d31_rise.json`;
- `nearA_new.py` writes `nearA_new_wanaka200.json`;
- `assert_c22_16_14.py` asserts each number below.

The set is every op-1 clearing sample over 0.471 at the base feed (slot feeds out), with the move kinds of §16.12.

**The §16.12 P failed.** The descent fix changed the after-step share by 0 mm (wanaka near A 2040.7 → 2038.4). So the after-step load is not the width read's descent gap.

**1. The rise by move kind, wanaka D26 → D31 (all over samples, 2222.8 → 2812.8 mm, +590.0):**
- on a step (descents and climbs): +36.1 mm. The sim reads these steps at a median woc of 0.70 in both runs, so D26 has them too.
- on the level move after a step: +187.4 mm.
- on other level moves: +366.5 mm.

The lead's classes:
- **(a) on a step the width read accepted: 36 mm, about 6 % of the rise.** The planner/sim width disagreement there is real but small in mm. It also cannot be read on this fixture: the sim cell is 1.0 mm (1/6 D), and the gap between the split threshold (c·D = 2.18 mm) and the bound (0.471·D = 2.83 mm) is 0.65 mm, less than one cell. The log has no per-step planner width.
- **(b) after a step: 187 mm**, and **(c) level: 367 mm.**

**2. Where the rise is, against the D26 plunge feet** (an `entry_plunge` move's end; within 2R in XY and DPP/2 in Z):
- D26 over at its own plunge feet: 905.6 mm. D31 over at the same places: 1296.9 mm (+391.3, two thirds of the rise).
- 318.1 mm of that lies where D26 plunged and D31 does not.
- Elsewhere: 1317.2 → 1515.9 mm (+198.7).
- Plunge feet: D26 883, D31 712.

**Reading.** A D26 split ends in a plunge at the step foot. The plunge bores a full-depth disc of R there, and the next passes cut into that hole. When A keeps the step down, the hole is not bored, and those passes meet the stock that the bore used to remove. That stock is the 2D plan's blind spot on terrain: the draped earlier passes leave it in the slab (G-FACESLAB). So the path rule's plunges hid part of the G-FACESLAB load, and A shows it again. A step's own load did not rise.

This is a reading of place, not a controlled test. D26 and D31 also differ by the D27–D30 changes.

**3. The trade: what each split buys (P15 numbers; the table is in `d31_rise.json`):**

| | splits | cycle | over 0.471 | per split |
|---|---|---|---|---|
| wanaka D29 v4 → D31 (width read added) | +2009 | +703 s | −509 mm | 0.35 s, −0.25 mm |
| wanaka D31 → D26 (split all) | +2262 | +819 s | −631 mm | 0.36 s, −0.28 mm |
| Corne D30 → D31 (same tree, descent fix only) | +191 | +15 s | −3.7 mm | −0.02 mm |
| Corne D31 → D26 | +897 | +236 s | −285 mm | 0.26 s, −0.32 mm |

- On wanaka the trade is nearly linear: every split costs about 0.35 s and buys about 0.26 mm of over-load. The splits that the width read makes buy no more than the ones it avoids.
- On Corne, the 191 extra width splits of D31 bought nothing (−0.02 mm each). The splits that the width read still avoids buy more (−0.32 mm each) than the ones it makes.
- **The honest answer:** on wanaka-like terrain, a stock-read split cannot hold the load without giving back most of the gain. To return wanaka to D26's over-load, the trade needs about all 2262 avoided splits, which is about 819 s, all of the cycle gain. The reason is the reading above: the load is not on the step, so a rule that reads the step cannot find it.

**What would hold the load and keep the cycle (proposals, no code):**
1. **The control first.** The same D32 tree with A off for the research arm, through a research env value `RS_CAM_RESEARCH_STEP_SPLIT=path_shape` (a few lines). Run wanaka and Corne. This gives A's own share on one tree; every number above is across builds.
2. **G-FACESLAB is the load.** The level and after-step share is stock that the 2D plan does not see in its slab. The fix belongs in the slice plan (it reads the drape's left stock as stock), not in the step rule. That is the G-FACESLAB row's fix, and it is generic.
3. **A cheaper bore than a lift.** At a kept-down descent foot whose next pass meets left stock, the lift-and-plunge gave a full-depth bore. A kept-down entry could cut the same disc without the lift (a fed bore at the foot, a helix of radius 0 at the step bottom). That is the Part B family ("never the entry's cut"). It would cost seconds where the planner stock shows the left stock, not at every step.

The operator ranks load-held first and cycle second. Until (1) and (2), my recommendation for the research arm is the path rule (A off), with Part B's keep-down travel kept. A holds the load on its own steps (36 mm), but by removing the plunges it shows load that only G-FACESLAB's fix removes.

## 17. G-FACESLAB: the 2D slice plan reads the stock that the drape leaves (design note, 2026-10-27; no code)

Scripts: `phase3/fix22/c22/drape_left.py` (writes `drape_left.json`) and `assert_c22_17.py` (asserts each number below).

### 17.1 What the defect is, measured

- **All of wanaka's D26 over-load (2222.8 mm over 0.471) is on the terrain level** (Z 8.62, draped; the 19.54 and 14.08 levels have none). On that level, nearly every pass is lifted onto the terrain.
- For each terrain-level clearing sample, the probe takes the highest EARLIER clearing sample of the same level within 2R. If that earlier tip stood more than the job tolerance (0.1 mm) above this sample's tip, the class is "an earlier pass above":

| D26, terrain level | an earlier pass above | none above | over rate |
|---|---|---|---|
| over 0.471 | 2144.4 mm (96.5 %) | 78.4 mm | |
| not over | 8580.1 mm (53 % of not over) | 7633.7 mm | |
| over rate | 20 % | 1.0 % | 20× |

- **Reading.** A draped pass rides on the highest ground under its disc. On the lower side of the disc it cuts nothing below its tip, and leaves a wedge there. The 2D plan marks the whole disc as cleared. A later pass at a lower tip meets that wedge on its side, across most of its disc, so the over-load follows. The shelf face of §16.11 is the same defect at a vertical step: the passes over the shelf are lifted, and the floor band beside it stays uncut.
- D26 over by axial engagement: 1283.6 mm deeper than 3 mm; 791.5 mm at 2 mm or less (thin wedges).
- D31 has the same structure (2754.5 mm in the "earlier pass above" class).

### 17.2 Why no step rule and no wall rule can fix it

- **A step rule (§16):** the load is not on the step (§16.14).
- **A wall rule** (read a face as a wall at its level): this works only where a face is vertical. Terrain has no walls. On a slope every pass leaves a wedge.
- **A rule that "clears a cell only to zero":** on a slope with a flat end mill, a pass clears only the cells within `tol / slope` of the rim on its high side. If the 2D grid stays binary and clears a cell only when nothing is left, every cell on a slope stays "material", and the plan cannot finish the level.

So the 2D plan must know the height of the stock, not only whether stock is there.

### 17.3 The design: a slab-height 2D grid (generic, in the 2D engine)

One shared part: **`SlabField`**. It is an optional input to the 2D adaptive engine (`adaptive/material_grid.rs`, `adaptive/search.rs`), built by the 3D slice for one region at one level. 2D operations do not pass it, so their output stays byte-identical.

1. **The grid holds a top per cell, not a flag.** `top(c)` is the stock top in the slab at cell `c`. At the start of a region it is the planner's 3D stock (`TriDexelStock` Z grid) at `c`, clamped to the slab top. A cell with `top(c) <= tip(c) + tol` holds no material for this level, where `tip(c)` is the tool tip at centre `c` (`level_floor_z(surface_hm(c), leave, z)`, the read `lift_to_level` already uses).
2. **A stamp lowers the tops to the tool.** A step from centre `p` with tip `tip(p)` sets `top(c) = min(top(c), tip(p) + h(|p - c|))` for each cell in the swept disc. `h` is the cutter's radial profile (the LUT the 3D stamp uses; 0 for a flat end). A lifted pass therefore leaves the wedge in the grid, as it leaves it in the stock.
3. **The engagement read counts the material that the tool cuts at its own height.** A step from `p` engages cell `c` only if `top(c) > tip(p) + h(|p - c|) + tol`. The swept width, the area and the pass-load ceiling (0.3626 at s 2, R 3) then read the same way as now. On flat ground, `tip` is the level everywhere, and the read is the binary read of today.
4. **What the plan then does.** A step down a slope meets the wedge of the pass above it and reads wide, so the direction search refuses it. A step UP the slope meets only fresh stock (its tip is above every wedge below it), and reads at the stepover. So the plan climbs a slope pass by pass, and a slope gets no side cut into a wedge. Beside the shelf face, the floor band keeps its top until a pass at the level cuts it. The next pass there reads that band, and the plan adds the passes the band needs.
- `tol` is the job's tolerance (the drape and the RDP read it). No new constant.

**Where each part lives, and who uses it:**
- 2D engine (`adaptive/`): the grid, the stamp and the read. The 3D slices of AgentSearch, ContourSpiral and Adaptive, and the research arm, all reach the engine through `clear_z_level_agent_2d_slice` or `clear_z_level_adaptive`. Each adopts the field in its own byte-parity step. The research arm adopts it first.
- `research_geodesic.rs`: the replay (`pass_load_readout`) reads the same grid, so the turn and slot records read the wedge with no extra code.
- ContourParallel (the operator's roughing baseline) does not plan by engagement. It can use the same field later to find the band that its draped rings leave, and cut it at the level. That is a separate design.
- The 3D stamp (`stamp_emitted_segment`) is unchanged. The field is the 2D image of what the 3D stamp does at one level.

**Cost:** one f32 per 2D planner cell, and one heightmap read per stamped centre. The planner already reads the heightmap at each lifted point.

### 17.4 How to measure it (pre-registration for the build)

1. **Shelf fixture** (`the_width_read_splits_a_wide_draped_step`, path rule): base over samples 5 (oneway) and 7 (spiral_pick).
   - P: the long-side overs (y ≈ ±15.8, 5 moves) go to 0.
   - The 7 mm island–shelf gap has no tool-centre width at the level when the gap is narrower than 2(R + leave). The field then reads it as material that no centre can reach. P: the plan does not cut it at full feed. The test reports it apart as "unreachable at the level".
2. **A new sloped-plane test** (a tilted plane, a flat end mill, one level): today each pass meets the wedge of the pass before it.
   - P: 0 samples over the exact bound with the field.
   - The plan climbs: each pass's tip is at or above the tip of the pass before it, within its disc.
3. **Wanaka and Corne fixtures** (research arm, A off):
   - P: the "an earlier pass above" over class (wanaka D26 2144 mm) falls by at least the part deeper than 3 mm (1284 mm). The probe in `drape_left.py` reads it.
   - The cycle cost is not predicted. The plan adds passes where wedges stand. The fixture measures the cost against D26 and D31.
4. **Byte parity:** 2D operations and every shipped 3D strategy are unchanged until their adoption step (`adaptive3d_emission_byte_parity`, the harness).

**Expected effect on wanaka's D26 2223 mm:** up to 2144 mm (96.5 %) is in the class that the field reads. It does not reach the 78 mm with no earlier pass above, or the planner/sim cell differences. I do not predict a number below that bound before the sloped-plane test runs.

### 17.5 Open points for the lead

- The 2D replay that C12's slot and turn feeds read changes with the field. The turn and slot counts then change. This is expected, but it is a change.
- Thin wedges (791 mm at 2 mm or less of axial engagement in D26) count fully in the sim's woc, as they do now. If the operator wants a thickness-weighted load instead, that is a load-model decision, not a part of §17.
- Fed bore at a descent foot (§16.14 (3)): parked, as ruled. With the field, a descent foot reads as a wedge like any other, so the bore is not needed.
