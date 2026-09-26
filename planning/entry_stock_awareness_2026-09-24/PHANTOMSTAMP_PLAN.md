# G-PHANTOMSTAMP — the adaptive3d planner stamps entries it later drops (2026-09-25)

Status: root cause found by reading code at b8983f5d; fix A (stamp on
commit) implemented 2026-09-26 in the working tree (uncommitted). See
RESULTS at the end.

## The strike (true; triaged at 0.5 / 0.25 / 0.125 mm cells)

Wanaka airrun project (`crates/rs_cam_core/tests/fixtures/wanaka_airrun_2026-06-01_2c908dca.toml`),
"3D Rough 6" (id 10, End Mill Ø6, agent_search, Global, plunge entry,
`min_region_cut_length_mm = 15`), move 1406: a rapid from Z 30 to 21.07 at
B = (89.640, 78.874), then EntryPlunge to 20.538. A cell wholly inside the
disc stands at the level-2 floor 22.4, so the rapid ends 1.33 mm into stock at
every cell size. The rapid floor is `plan_entry`'s read of the PLANNER stock
(clearing.rs:727-728) = 20.570, + 0.5 (path.rs:1505-1509).

## Root cause: `coalesce_redundant_entries` drops entries after they were stamped

- `push_segment_with_stamp` (clearing.rs:848-895) runs `plan_entry`, stamps
  the segment into the planner stock and moves `last_pos` at push time. For a
  `RapidWithFloor` the stamp is a column from `descent_floor` to the draped
  entry z (clearing.rs:651-685); a zero-length-XY stamp is a full disc at the
  lowest depth (stamping.rs:847-851), lowering `conservative_top` on every
  fully covered cell.
- `coalesce_redundant_entries` (clearing.rs:2871-2919) runs after the whole
  level is planned (2839-2844) and deletes entry i when the next non-marker
  segment is another entry. Its comment (2677-2681) says the dropped plunge
  "gets re-stamped at the second's XY"; false: the two sit at different XY.
  The dropped column stays in the planner stock, and every later
  `plan_entry`, the bool grids and the waterline pass read it.
- Consecutive entries come from `clear_one_region`: an air run ending at a
  large-Z step (2517-2537 then 2542-2558); a one-point engaged run that is
  never pushed (2500-2502) between two large-Z rapids (the dropped entry sits
  on ENGAGED material); a wall climb, where every step is steeper than
  `PLUNGE_SLOPE_LIMIT` 0.3 (2476); a 2D Rapid/Link entry (2600-2646).
- Wanaka fit: a dropped column at a rim point ~0.75 mm south of B fully
  covers (90, 76); B's disc then reads ~20.57; floor 21.07 over real 22.4 =
  1.33 mm. Level 3 never re-plans that material (its bool grid says it is
  gone); it is cut only after the level (moves 1623-1629). `last_pos` was set
  to the dropped entry, so B's stay-down proof starts there and the emitter
  (path.rs:1516-1521, 1189) rejects it: the retract at 1404.

Refuted: emission order (adaptive3d `.without_rapid_reorder()`, catalog.rs:609-611;
both gates dressup_apply.rs:504-507, 803-806; `segments_to_toolpath` walks in
order, path.rs:1465); footprint mismatch (drape, RDP, blend and the entry
column are mirrored; `conservative_top` lowers only on full coverage); region
order (plan order is emission order, path.rs:973-1112; clearing.rs:1864-1871).

Same class, small: the emitter feeds a Cut from the tool position to
`blended[1]` (path.rs:1661 `.skip(1)`) while the mirror stamps from
`blended[0]` (clearing.rs:619), at most one path step; a Cut with len < 2 is
stamped as a point (clearing.rs:620-621) but not emitted (path.rs:1637-1639);
a stray `*last_pos = Some(path_3d.last())` (clearing.rs:2459-2461) gives an
air-first path's entry a proof from the path end (costs a retract only);
the boundary clip leaves ≤ ~0.25-0.5 mm next to the silhouette.

## Fix: A, stamp on commit (chosen)

In the agent slice (`LevelEmission`), push an entry WITHOUT stamping; keep it
pending. On the next non-marker segment: a Cut or Link → run `plan_entry`,
stamp the pending entry, set `last_pos`, continue; another entry → delete
the pending one (never stamped, `last_pos` unchanged) and make the new one
pending. Markers pass through; flush at level end (today's trailing-entry
behaviour); keep the `min_region_cut_length_mm > 0` gate.
`coalesce_redundant_entries` then removes nothing: keep it as a
`debug_assert` that it returns 0. Fold in the three small mirror fixes above
(Cut from the actual tool position; len < 2 Cut mirror a no-op; remove the
stray `last_pos` write).

Correct because the stamped set equals the emitted set, in order, so the
planner stock at each entry is exactly what earlier emitted segments cut:
floors only rise against today. The pending entry's `plan_entry` reads the
same stock it would have at push time (only markers intervene).

Rejected: B, snapshot and replay per level (doubles stamping; air/engaged
decisions inside the level were still made on the phantom stock); C, stop
coalescing (reverts F-038, est. +2-5 % time).

Cycle time: expected about neutral (±1 % of rivmap100 839 s Global / 789 s
By Area, unmeasured): floors rise near phantom columns (a few short pecks),
proofs start at the real position (more keep-down links). Measure with
`rough-score` Global and By Area.
Blast radius: AgentSearch and ContourSpiral. Re-bless
`adaptive3d_emission_byte_parity` case by case; `adaptive3d_entry_coalescing_f038`
keeps its bound; re-run `agent_search_coverage`, the F-029/F-031 heavy tests,
`perf_golden_sim_metrics`.

## Sentries

1. `adaptive3d_planner_never_ahead_of_emitted_path` (new): 60×60 plate at
   z 0, a centred 20×20 boss 12 mm tall with 45° chamfered walls (slope 1 >
   0.3, so wall steps split into rapid runs), stock top 15; flat Ø6,
   AgentSearch, Global, plunge, dpp 2.6, stepover 2.2, leave 0.5,
   `min_region_cut_length_mm` 15, no boundary. Precondition: the
   `coalesced_entries_f038` counter > 0 on some level (not vacuous).
   Assert (1) replaying the emitted toolpath on a fresh stock at 0.5 mm, no
   cell where the planner's final top (`debug_adaptive_3d_segments_for_f029_probe`)
   is below the replay by > 0.25 mm; (2) rapid_collisions == 0 at 0.25 and
   0.125 mm. Show red before the fix.
2. Wanaka (heavy): generate the chain to id 10; rapid_collisions for "3D
   Rough 6" == 0 at 0.25 and 0.125 mm; the entry at (89.64, 78.874) descends
   no lower than 22.4 + 0.5 before it feeds.
3. Pre-fix instrumentation check: log each coalesced entry's (x, y, z); at
   level 3 of 3D Rough 6 expect one within ~2.65 mm of (90.0, 76.0), ~0.75 mm
   from B, z ≈ 20.57.

## RESULTS (2026-09-26, fix A landed in the working tree, not committed)

Measured only; each line names its command or test.

- Pre-fix instrumentation (§Sentries 3; temporary `eprintln!` in
  `coalesce_redundant_entries`, removed): Wanaka "3D Rough 6" level 3
  (Z 19.800) coalesced 77 entries; one at (89.245, 77.427), raw z 19.800
  (the level Z; the draped z the column was stamped to was not logged),
  1.614 mm from (90.0, 76.0) and 1.500 mm from B (89.640, 78.874).
- Wanaka sentry `adaptive3d_wanaka_rough6_no_phantom_rapid_g_phantomstamp`
  (release, `--ignored`): pre-fix RED, rapids at the strike XY
  `[(1405, 30.0), (1406, 21.070)]` below 22.9. Post-fix green: no rapid
  ends at the strike XY (the entry is no longer emitted there); "3D Rough
  6" rapid_collisions at 0.25 mm = 0 (0 in the chain up to it). 420 s.
- `p1_headless_ab_wanaka` (release, 0.5 mm): rapid_collisions = 0 (was 1,
  move 1406). `BASELINE_RAPID_COLLISIONS` lowered 4 -> 0. 3D Rough 6
  total 278.9 s (cutting 152.7, entry 25.1, linking 17.4, rapid 83.8).
- Boss sentry `adaptive3d_planner_never_ahead_of_emitted_path`: pre-fix
  RED (pre-fix `clearing.rs`/`path.rs` put back): plan stopped at Z 9.8,
  1 cell 1.219 mm below the replay at (-16.50, -25.50); whole plan 2
  cells, 0.252 mm at (20.00, -20.50); 94 entries coalesced. Post-fix: 0
  cells at every stop (Z 12.4 / 9.8 / 7.2 / 4.6 / whole), 87 entries
  replaced unstamped. Its rapid half (0 collisions at 0.25 / 0.125 mm) was
  green before and after.
- `adaptive3d_emission_byte_parity`: only `agent_search` moved (358 -> 352
  moves; retracts 51 -> 45, linking rapids 32 -> 20, 1500 mm/min feeds
  198 -> 210). First change at move 35: a retract + rapid + plunge became a
  keep-down feed link. Re-blessed. `contour_parallel`, `adaptive`,
  `contour_spiral` unchanged.
- rivmap100 (`arm.sh`, 0.5 mm, dpp 8; its 3D Rough is ContourParallel, so
  only the cut/link mirror changes reach it): Global 839 s (moves 9279,
  entry 417, rapid 98) and By Area 789 s (moves 8707, entry 389, rapid 85),
  identical to master.
- rivmap100 with `--set 1.clearing_strategy=agent_search` (the arm the fix
  changes), master -> fix: Global 920 -> 980 s (+6.5 %; moves 8721 ->
  9343, entry 582 -> 616, rapid 232 -> 222, vol 42883 -> 43287 mm³); By
  Area 862 -> 913 s (+5.9 %; moves 8048 -> 8551, entry 547 -> 571, rapid
  221 -> 209, vol 42144 -> 42594 mm³). Not the ±1 % the plan expected: the
  planner now plans the material the phantom columns hid (+400-450 mm³).
