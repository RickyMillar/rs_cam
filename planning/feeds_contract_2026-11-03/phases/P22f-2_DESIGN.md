# P22f-2 design: the band-edge arcs and the regions that still sweep (2026-11-10)

Read-only design. No file under `crates/` changed. Base: the P22f-1 tree (release CLI built
from it, copied to the session scratchpad before 6b took the tree). Evidence: the P22a wanaka
arm of P22f-1 (`P22f-1.md` 3.2), the release CLI (`project --sim-artifact`, plain and with
one switch), the emitted G-code, C22 (`planning/adaptive_research_2026-10-23/P3_C22_3d_turns.md`
§11.10-11.12) and `DEFECTS.md` (G-ARCENDHOOK).

## 0. Summary

- **What emits them.** The 25 band-edge moves are the ends of geodesic arcs of region 1 at level
  15.5 (model floor Z 16.5). Each arc runs diagonally down to the foot of a 4 mm step in the model
  (toolpath y 119.3; the floor is Z 16.5 below it and a terrace at Z 20.5 above it), climbs the
  step in one capped D65 3D split move, goes on along the terrace to y 111.8 (the grown 2D edge),
  and retracts. Region 2's end at (94.1, 42.8) lifts straight to Z 26. No semantic item names the
  piece (the slice swallows the 2D markers; only the level item covers them).
- **Why the planner read them under the load.** Since P22f-1 the grown 2D domain covers the
  terrace: centres whose floor (Z 20.5) is above the level, where stock stands above the floor.
  The 2D plan treats the level as flat and holds every cell its arcs sweep as cut. In 3D the
  lift drapes the terrace parts to Z 20.5 and splits the step crossings, so stock at the step foot
  that the 2D grid holds cut stays standing, and a later arc that runs down to the foot meets it
  across its disc (ae 13.25 at ap 9, real removal). The 3D stock-read split (`step_load`) reads
  only steps that the shape rule flags (steep or a large Z change); a flat step at the level is
  never read on the 3D planner stock, so it goes out uncapped. This is C22's H1 (2D centres in a
  drape area, lifted; §11.10) on a 7.5 mm wide terrace, with the G-ARCENDHOOK pattern (an arc end,
  then a lift; §11.12). Before P22f-1 the sweep removed that stock first.
- **Measured** (CLI `--sim-artifact`, ids 0 and 1, no modulation): in the band window along the
  step foot (y 111.8-128.6, X 42-316, Z 16.5) 207 samples past `c D` plus both cells, 99 of them
  real removal. Detect Flat on idx 1 does not change it (it adds a level at 16.55, not at the
  terrace). The same run shows a worse sample outside the band: ap 10, ae 17.76 at the full feed
  (2.19 kW at the planner feed; open question 3).
- **Recommendation: (b) then (a').** (b) completes the D65 rule: every fed step of a geodesic cut
  is read on the 3D planner stock by the one load (`step_load`), capped at `c / L` up to a slot
  and split above. It reuses existing code, catches the band-edge ends and the full-slot sample
  outside the band, and keeps every finding. Then (a'): split a region by floor height and plan
  each floor piece on its own grown outline, so no 2D centre stands in a drape area; that removes
  the width, not only the feed.
- **The P22f-2 regions** (op 20 split, op 21 closing guard): (b2) plans the split pieces and
  leaves the neck to the rest pass with a finding; (c) caps the sweep at `c` with a finding where
  no piece can be planned (ruling 2).

## 1. Diagnosis

### 1.1 The piece (question 1)

The semantic items over the 25 moves are only the level (`Global Z 15.50`), the operation and
the boundary clip; no `FeedCap` item covers them (they are uncapped) and the slice emits no
per-piece marker. So the piece is read from the moves. The G-code frame is the toolpath frame
plus the stock origin offset (18.5, 8.5), and G-code motion line `i + 1` is move `i`.

| Band-edge move (end) | Next moves (toolpath frame) |
|---|---|
| 5844 (100.9, 119.3, 16.50) | (102.3, 118.5, 20.50) capped climb; (108.3, 115.3, 20.50); (115.6, 111.8, 20.50) |
| 6453 (80.7, 119.3, 16.50) | (81.3, 118.9, 20.50); (87.2, 114.8, 20.50); (92.0, 111.8, 20.50) |
| 6686 (65.2, 119.3, 16.50) | (66.3, 118.4, 20.50); (74.8, 111.8, 20.50) |
| 7414 (245.0, 119.3, 16.50) | (244.8, 119.2, 20.50); (235.8, 115.1, 20.50); (226.8, 111.8, 20.50) |
| 8026 (267.2, 119.3, 16.50) | (265.8, 118.4, 20.50); (259.9, 114.8, 20.50) |
| 8630 (285.9, 119.3, 16.50) | (285.3, 118.8, 20.50); (280.8, 115.3, 20.50); (275.8, 111.8, 20.50) |
| 12228 (94.1, 42.8, 15.50), region 2 | lift to Z 26.00 at the same XY |
| 15437 (94.1, 42.8, 11.00), region 2 one level down | lift to Z 25.75 at the same XY |

The arcs are the field's wraps (diagonal level-set arcs, spaced along the step), not a cleanup
pass: they cross the step and stop at the grown edge y 111.8. The climb at the step is a D65 3D
split step (`CappedSplit3d`, feed about 2540 mm/min); the run ends with a retract.

### 1.2 Why the planner read them under the load (question 2)

- **The 2D domain.** The region at a level holds every cell where stock stands above
  `max(drop-cutter + leave, z_level)` (`build_material_bool_grid`). The terrace (floor Z 20.5,
  stock to 25.5) is part of region 1 at level 15.5. Before P22f-1 the 2D domain was the smoothed
  region inset by R, which ended near the step; the sweep cut the step foot. After P22f-1 the
  grown domain reaches 1.51 mm inside the region edge, so it covers the terrace band
  (y 111.8-119.3, 7.5 mm wide in tool-centre space).
- **The 2D plan holds the band cut; the 3D path does not cut it all.** The 2D plan is made once
  per level on its own flat grid. In 3D: the terrace parts of an arc are lifted to Z 20.5 and cut
  only above the terrace; their discs (R 9) reach back over the lower floor near the foot and cut
  only above Z 20.5 there; each crossing of the step is a draped step that the stock-read split
  caps (one climb) or splits into an entry. The stock that the 2D grid then holds cut at the
  foot stands up to the full height (ap 9 at the samples, stock top 25.5 over floor 16.5).
- **No 3D read on a flat step.** `admits_capped_split` and the split read `step_load` only when
  `shape_split` holds (|dz| over `max(1.1 dpp, 1)` or slope over `PLUNGE_SLOPE_LIMIT`). A flat
  step at the level is emitted as the 2D plan has it, at the plan's feed. The 3D planner stock
  (stamped by the emitted moves) knows the foot stock stands, but no rule reads it there.
- **Not measured: the stock itself.** No public door gives the planner stock or the 2D grid at a
  point in time (`Adaptive3dSegmentsResult::final_material_stock` is crate-private; the simulation
  result keeps a mesh). The cut trace gives the samples; the band window (0) and the ap/ae of
  each sample stand for the stock read. An instrument for the exact read: a `test-support` door
  that returns, per level, the 2D grid the plan used and the 3D planner stock tops after the
  level (`debug_adaptive_3d_segments_for_f029_probe` returns the final tops only).

### 1.3 Does C22 / C31 name this case? (question 3)

Yes, twice, both parked.

- **H1, the drape ring** (§11.10): "a 2D pass in the drape ring, lifted ... leaves a layer of level
  stock that the 2D grid holds cut. The next pass outward meets that layer across its disc." On
  the C12 pocket the drape ring was one mesh step wide and the probe refuted H1 (§11.12). On
  wanaka the drape area is a 7.5 mm terrace, 4 mm high: H1 in kind, at a size C12 did not have.
- **G-ARCENDHOOK** (§11.12, `DEFECTS.md`): an arc's last segment cut into standing stock, then a
  lift; "the shipped sweep removed that stock before C31". Wanaka's ends match the pattern
  (arc end, then a climb or a lift); their segments are long (5-11 mm), not 1-2 mm hops.
- The parked fix (§11.11): grow from the level's flat floor F (cells with `surface + leave <=
  z_level`), leave the drape area to the waterline, guard `w_max <= c D - cell`. On wanaka
  `w_max` = 7.5 mm against `c D - cell` = 5.99 - 1.5 = 4.49 mm: the guard refuses, so region 1
  would keep the sweep. §11.11 as written does not serve a terrace.

## 2. Options (question 4)

| Option | Mechanics | Band-edge samples (past both cells / real) | Worst real sample | Cost |
|---|---|---|---|---|
| (a) the planner grid marks the band uncut | The 2D plan holds the cells of a step that the lift leaves (lifted above the level by more than the leave, or split) as standing: replan, or replay the plan on a grid that the lift feeds back. The 2D plan is made before the lift, so this needs a lift-aware replay per piece | predicted 0 at the foot; the replay then splits or caps the arcs that meet it | planned steps <= 0.80 kW | large: a second replay per piece with 3D feedback |
| (a') floor pieces (§11.11 made general) | Split the region by floor height (the flat floor F, and each terrace T as its own piece at its own floor, read on the drop-cutter grid); grow and plan each piece on its own outline. No 2D centre stands in a drape area; the drape ring between pieces (one slope width) is the waterline's | predicted 0 | floor ap 10 at `c D`: 0.80 kW; terrace ap 5 at `c D`: 0.40 kW | medium: one more partition in `clear_one_region`; the closing guard and the clips per piece |
| (b) the D65 rule on every fed step | In the lift loop, read `step_load` on the 3D planner stock for every step of a geodesic cut (not only `shape_split` steps); over `c`: cap at `c / L` (`capped_step_factor`, at most a slot); over a slot: split, as today | unchanged in count (207 / 99), each at a capped feed | the band end at ap 9, ae 13.25: `c / L` = 0.452, F 3833: 0.91 kW. The full-slot sample outside the band (ap 10, ae 17.76): F 2859: 1.22 kW, still over 1.125 (the P20g modulator lowers it further) | small: one call per step; perf to measure on wanaka (about 20 k moves) |
| (c) a smaller grow (stop at the flat floor) | Grow by `R - m` from F only (as (a') without the terrace piece) | predicted 0 at the foot | the waterline at the step meets `cell + m` (1.75 mm) at the foot; the terrace stock (7.5 mm wide, ap 5) is cut by no level of this op: it goes to the rest op (op 2, D 6). If a waterline met it: ae 9.0 (0.50 D) at ap 5, F 8475: 0.57 kW | small; moves volume to the rest op |
| (d) Detect Flat on idx 1 (no code) | `detect_flat_areas = true` adds levels at flat shelves | MEASURED: 207 / 99, unchanged (the added level is 16.55, the floor, not the terrace) | 1.44 kW (move 7329, the band end) | none; no effect |

Power: the P20i true-width law, `P = Ks ap ae F / 6e7 + F_edge ap (pi D n)(z psi / 2 pi) / 6e7`,
`psi = acos(1 - 2 ae / D)`, `Ks` 58.516, `F_edge` 4.595, D 18, n 18000, z 2; available 1.125 kW.

## 3. Predictions on idx 1 (question 5)

| | Today (P22f-1) | (b) | (a') | (b) then (a') |
|---|---|---|---|---|
| Band-edge samples past both cells (CLI window) | 207 (99 real) | 207 (99 real), all capped | about 0 | about 0 |
| Uncapped fed samples past both cells with real removal (P22a classes OTHER + joint + draped) | 272 + 54 + 101 | about 0 (each is read and capped) | about 54 + 101 (the draped and joint classes are not only terraces) | about 0 |
| Worst real sample at the planner feed | 2.19 kW (ap 10, ae 17.76, outside the band) | 1.22 kW (that sample capped at `c / L`) | 2.19 kW (unchanged outside the band) | 1.22 kW |
| Worst at the band end | 1.45 kW | 0.91 kW | <= 0.80 kW | <= 0.80 kW |

The P20g modulator then holds every move at or under 1.125 kW, as today; the planner's job is to
not ask for the width (a') and to state the feed it needs (b).

## 4. Pins at risk and the sentry (question 6)

- (b) changes `Move::feed_cap` on geodesic fixtures where a flat step reads over `c` on the 3D
  stock: G-OPGOLDEN `adaptive3d/*` geodesic rows (moves hash; `Move`'s debug text prints the cap)
  may join the 46 known rows (re-bless with the reason, per row); `adaptive3d_emission_byte_parity`
  (geodesic case) may move; the D65 sentry's claims hold (fewer or equal entries; every lowered
  move carries its cap); `geodesic_adaptive_keeps_the_research_geometry_s7_4` holds geometry
  ("feeds aside"); the C12 judge can only improve. The 2D door and `the_geodesic_preset_keeps_its_plan_s7_3`
  do not move (3D only).
- (a') moves the plan of any region with more than one floor height: the C12 pocket has none
  (vertical walls), the shelf fixture has one (the shelf), wanaka many. `adaptive3d_plans_every_slice_piece_g_adapt3dpieces`
  (pieces per region) must read the floor split as pieces.
- New sentry `a_geodesic_arc_end_reads_the_stock_p22f2`: the shelf scaled by `D / 6` with an
  18 mm tool (the P22f-1 sentry's fixture: its shelf is a 7.5 mm terrace above the level) through
  `ProjectSession` and a simulation; claim: no fed sample of an uncapped clearing move reads past
  `c D` plus both cells with real removal (the P22a removal check). The classifier moves from the
  P22a test into `tests/common/` so both read one definition. Predicted red today, green after (b).

## 5. The regions that still sweep (question 7)

| Op | Level | Case | (b2) plan the band | (c) cap the sweep with a finding |
|---|---|---|---|---|
| 20 Front bulk rough 18 mm | z 17.168, regions 1 and 2 | the shortfall inset split the outline | Accept the split: grow and plan each inset piece; the part of the smoothed region that no piece's centre region covers (the neck, narrower than twice the shortfall) goes to `leave_piece` (the D44 rest regions) with the finding "a neck of region R at z left to the rest pass" | Cap at `c / L` with `L` = 1 (a fresh slot): F 2821; ap 4.17 (the level spacing 21.334 - 17.168): 0.53 kW against 0.94 kW at the op feed; finding "the band of region R at z is cut as a full slot at the capped feed" |
| 21 Front 3D rough (D 6) | z 19.468 region 1; z 13.936 region 13 | closing guard | The guard measures the closing of gaps narrower than about 2R; plan the region on the smoothed outline (no grow) and leave the band to the rest pass where it is narrower than 2R | the same cap and finding at the 6 mm tool's own `c` |

Ruling 2 orders (b2) where a plan exists and (c) with a finding where none does. (b2) for the
split case adds no new rule (the pieces already exist; only the neck is left). (c) is the net.

## 6. Phase split and blast radius

- **P22f-2a**: (b), the D65 rule on every fed step of a geodesic cut (`adaptive3d/clearing.rs`,
  the lift loop: call `step_load` where `shape_split` is false too); the new sentry; G-OPGOLDEN
  rows read and re-blessed one by one with the reason. `rg` today: `admits_capped_split` has 1 production
  call (`clearing.rs`) and 6 test asserts (`adaptive3d/tests.rs`); `step_load(` has 1 production
  call (`clearing.rs`, the lift loop) and 3 test calls; C4 reads `step_load_on`.
- **P22f-2b**: the op 20 / op 21 regions: (b2) for the split, (c) as the net, with findings.
- **P22f-3**: (a') floor pieces, after a measured design on the shelf and wanaka.

## 7. Open questions

1. Is (b) acceptable as a first step, given the lead's line "a feed cap hides a width the planner
   chose"? Recommendation: yes, as the net: it states the feed each step needs on the stock the
   planner knows, and (a') then removes the width. Without (b) the band-edge ends run at the full
   feed until (a') lands.
2. (b) costs one `step_load` per step. Recommendation: measure the generation time of idx 1
   before and after (debug and release); if it is over 10 %, read only steps that end within R of
   a lifted or split step of the same level.
3. The worst sample of the CLI run (ap 10, ae 17.76, F 8475, 2.19 kW at the planner feed, at
   (138.8, 175.2, 15.5), move 1612 of that run) lies outside the band and outside every capped
   class. Recommendation: read it with the P22a classes in the P22f-2a run; (b) caps it.
4. The exact stock read (question 2) needs a `test-support` door. Recommendation: add
   `debug_level_plan_probe` (the 2D grid per level and the 3D planner tops after each level) in
   P22f-2a, test-only, so the sentry can also assert the 2D/3D agreement at the foot.
5. Detect Flat did not detect the terrace. Recommendation: read why in P22f-3 (its shelf rule may
   need a minimum area or reads the floor only); (a') needs the same floor partition.

## 8. Lead rulings (2026-11-10 night)

1. (b) first, as the net: the D65 rule on every fed step of a geodesic cut (`step_load` on the
   3D planner stock; cap `c / L` up to a slot, split above). It states the feed each step needs
   on the stock the planner knows. (a'), the split of a region by floor height with one grown
   outline per floor piece, removes the width and follows as P22f-3 with question 5 (why Detect
   Flat misses the terrace).
2. Measure the generation time of idx 1 before and after (debug and release) in P22f-2a. Over
   10 %: read only the steps that end within R of a lifted or split step of the same level.
3. Yes: the P22f-2a wanaka arm reads move 1612 (2.19 kW at the planner feed) with the exact
   classes and states what (b) does to it.
4. Yes: a `test-support` probe of the 2D grid per level and the 3D planner tops after each level,
   so the sentry asserts the 2D/3D agreement at the foot. Test-only; no product reader.
5. P22f-3.
6. Section 5 stands for ops 20 and 21: (b2) plan the pieces and leave the neck to the rest pass
   with a finding that names the region and the level; (c) the `c / L` cap with a finding where
   no plan exists. Never a full-slot sweep at the op feed.

Order: P22-0c-6b (in the tree now), then P22f-2a ((b), the probe, the sentry
`a_geodesic_arc_end_reads_the_stock_p22f2`, the generation-time measure, the wanaka arm), then
P22f-2b (ops 20 and 21), then P21a / P21b, then the wanaka read, then P22b. Pins: (b) may add
G-OPGOLDEN geodesic rows (the feed cap is in the move hash) and move `emission_byte_parity`'s
geodesic case: list each moved row with its reason before the bless; the 2D door and s7_3 must
not move.
