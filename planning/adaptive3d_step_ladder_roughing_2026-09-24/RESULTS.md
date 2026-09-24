# Phase 1 arms on rivmap100 (2026-09-24)

Measured through the GUI MCP, simulation 0.5 mm, accel time from
`rs_cam_cli nc-time --max-feed 6000 --rapid-feed 10000` (built-in
shapeoko_xxl_ricky_tuned kinematics: accel X/Y/Z 500/500/270 mm/s²,
junction deviation 0.020 mm). Face alone = 605 s; rough time = export total −
605 s. Scallop and Project Curve disabled. Artifacts:
session scratchpad `arms/` (not in the repo).

Model depth: after Face the stock top is Z 12; the rough goes to about
Z 2–4. So the whole rough is about 8–10 mm deep. B10 is one level.

| arm | rough accel s | moves | cut mm | rapid mm | entries | peak axial DOC mm | vol/accel s (Face+Rough) mm³/s |
|---|---|---|---|---|---|---|---|
| A-regen (DPP 2, Global) — baseline | 1741 | 10 442 | 26 613 | 8 253 | 268 | 2.0 | 31.5 |
| A2 (DPP 2, By Area) | 1481 | 8 905 | 23 705 | 5 681 | 219 | 2.0 | 35.2 |
| B5 (DPP 5, By Area) | 871 | 5 373 | 15 121 | 3 017 | 125 | 5.07 | 48.6 |
| B10 (DPP 10, By Area) | 635 | 3 922 | 9 839 | 2 191 | 84 | 8.0 | 57.6 |
| Bx (10 @ leave 5.5, then 5 @ 0.5) — INVALID as a ladder proxy | 944 | 5 798 | 15 772 | 3 788 | 152 | 5.07 | 46.3 |

Notes:

- The Phase 1 gate passes: B5 and Bx beat A on time.
- The pre-session arm A (19 624 moves, 3528 s) did not reproduce. A
  regenerate with the same params gives 10 442 moves, twice, the same.
  Cause not established (a different prior stock is likely, not verified).
  A-regen is the baseline.
- By Area alone saves 260 s (15 %).
- The removed volume (Face + Rough) falls from 73 966 mm³ (A-regen) to
  71 428 mm³ (B10), about 3.4 %. Larger steps leave more for the finish.
  Section 4's "same volume" assumption does not hold; the finish time
  must be part of the score.
- Load: the static rigidity rule passes B10 (8.0 of 9.0 mm, 0.58 kW,
  53 µm). The simulation's `entry_load` finding grows with the step
  (critical: A-regen 26 samples, peak 1.14 mm; B10 560 samples, peak
  5.96 mm). The load gates skip entry spans.
- Bx is not a valid proxy of the ladder. With leave 5.5 on an 8 mm deep
  model the first operation cuts almost nothing, and the second one repeats
  B5. Do not cite its 944 s.
- B10 confirms F3. The model is 8–10 mm deep, so B10 is one level, and the
  drape takes the whole depth next to the walls (peak axial DOC 8.0 mm,
  `entry_load` 560 samples to 5.96 mm). The fit rule exists to stop that
  bite near the walls and give those bands to 5 mm passes.
- This model is too shallow to test the ladder's nesting: [10, 5] is one
  coarse slab here. The synthetic fixture (≥ 25 mm deep, several coarse
  slabs, [10, 5, 1]) is the only test of the nesting.
- Pre-registered bound for Arm C ([10, 5], By Area) on rivmap100: rough
  accel time between B10 (635 s) and B5 (871 s). The ladder's gain over B5
  is bounded by B5's time in the open floors. The ladder's gain over B10 is
  in load (no 8 mm wall bites) and in the finish, not in rough time.
- Radial engagement is not measurable at 0.5 mm for these roughs (31–69 %
  blind). The whole-cycle engagement number waits for a finer grid.

## Finish check (Scallop after each rough)

Scallop generated against a 0.2 mm simulation of each rough (the planner's
own pre-simulation); scored at 0.5 mm; accel time from `nc-time`.

| arm | rough accel s | scallop accel s | rough + scallop accel s |
|---|---|---|---|
| A-regen | 1741 | 5598 | 7339 |
| B5 | 871 | 5568 | 6439 |
| B10 | 635 | 5580 | **6215 (−15 %)** |

- The finish time does not change with the rough. The extra stock that
  larger steps leave costs the scallop nothing measurable.
- The finished part is the same: Face + Rough + Scallop removed volume
  89 187.7 / 89 188.4 / 89 188.1 mm³; uncut, truncated and untouched areas
  0.0 on every arm; the finish renders differ by 0 pixels over 20.
- So on rivmap100 the zero-code B10 + By Area already takes the whole gain
  in time. The ladder's value on this model is load (no 8 mm bites at the
  walls), not time.

## Follow-up found during the finish check

- G-MCPMODAL: an MCP `generate_toolpath` on a rest operation (Scallop after
  3D Rough) stops behind the GUI pop-up "Simulation resolution … Use 0.200 mm
  / Cancel". MCP cannot answer it: `cancel_generation` says there is nothing
  to cancel, and `set_ui_view modal:none` does not close it. The generate
  waits until a person clicks. Owner: viz MCP (not this package).

## The scoring instrument (Phase 0, landed 1f3e6f3c + a0447f46)

`rs_cam_cli rough-score <project.toml> --toolpath <i> --resolution 0.5
[--set <i>.<param>=<value> …]` prints one JSON record per toolpath. From
Phase 2 on, every arm uses this one instrument.

Parity rule (operator, 2026-09-24): the GUI, MCP and CLI must give IDENTICAL
numbers for the same project state. The first comparison (MCP B5 871 s
against `rough-score` 902 s) was NOT paired, so it proves nothing either
way:

- the CLI read the on-disk TOML, which has `region_ordering = "global"`;
  the MCP arm B5 used `by_area`;
- `nc-time` replays the G-code with the built-in preset; `rough-score`
  integrates the IR with the project's machine profile.

A paired check (the GUI state saved to a scratch TOML, then `rough-score` on
that file) is queued. With identical inputs, any difference is a defect.

Corrections to PLAN.md section 4 found while it was built:

- On a machine with a `kinematics` block, the published simulation
  `total_runtime_s` is already the accel time. `rough-score` re-accumulates
  the samples, so its `sim_nominal` block is nominal length / feed.
- `sim_nominal.cutting_runtime_s` counts entries and links as cutting;
  `accel_time.cutting_s` holds only clearing/finishing moves.
- The simulation reads the feed before modulation; `accel_time` reads the
  modulated feed that the export uses. Each block names its `feeds_basis`.

First reading (rivmap100 on-disk TOML, index 1, `depth_per_pass=5`, Global): accel total 902 s; entry moves 310 s (34 %); 124 plunge
entries against 15 helix entries although the operation asks for helix;
rough-only removed volume 47 611 mm³. The entry share is a lever of its own
(cause not checked: a helix that falls back to a plunge, or the untagged vertical descents); it is outside this package.

## Parity check: GUI/MCP against `rough-score` (paired)

The GUI state (DPP 5, By Area) was saved to a scratch TOML and scored with
`rough-score` on that file.

- With the same prior-stock resolution, the move count is identical (5373)
  and the GUI toolpath `total_runtime_s` equals `rough-score`
  `accel_time.total_s` to 13 digits (813.711 s). The shared session works.
- G-RESTRES (defect): the same parameters give different geometry
  depending on the resolution of the LAST simulation. The GUI generated the
  rough from a Face stock simulated at 0.2 mm (left by the scallop's
  pre-simulation) and got 7925 moves; at 0.5 mm, 5373. The generated result
  does not record the stock resolution it read, and a simulation at another
  resolution does not mark it stale. This also explains the pre-session
  arm A that did not reproduce (19 624 against 10 442 moves). The CLI takes
  the resolution as an argument; the project file does not carry it.
- G-RESTSTALE (defect): at the restore, the scallop generated 245 584 moves
  against the A rough, which is B5's count: it read a stale stock snapshot
  and nothing flagged it. See also G-FRESHNESSDISAGREE
  (`arch_consolidation_2026-09-09/STATUS.md`).
- Instrument defect (being fixed): `rough-score` printed a second,
  re-accumulated nominal time base under look-alike names
  (`cutting_runtime_s` 760.90 against the GUI's 703.12) and no cut / rapid
  distance. It must print the GUI's published fields, same names, same
  values.
- MCP gives removed volume only for the whole project, not per toolpath.

Instrument fixed (9699f0d5): `rough-score` prints the core
`ToolpathDiagnostic` and `SimulationToolpathCutSummary` as the session
publishes them. On the parity TOML it matches the GUI on moves (5373), cut
and rapid distance, `total_runtime_s` (813.7110364401705) and
`cutting_runtime_s` (703.1247). A test (`rough_score_publishes_the_gui_numbers`)
holds the parity. Follow-up G-MCPCUTROW: the MCP `get_cut_trace`
`toolpath_summaries` row leaves out `average_engagement`,
`total_removed_volume_est_mm3`, `peak_axial_doc_mm` and `per_kinematics`,
which the core struct has.

## Phase 2 result (2026-09-24)

State: the step ladder is in core, on branch
`worktree-agent-acc0c9c065ae21db6`, not merged. The GUI cannot set
`coarse_steps` until Phase 4.

### Commits

| Commit | Content |
|---|---|
| `102987b2` | BREAKING: deletes `fine_stepdown`, `mill_shallow_areas`, `shallow_angle_deg`, `shallow_stepdown` and `ShallowTier`. Adds `coarse_steps` (serde default, not written when empty), `plan_step_ladder` (the nested slab schedule of §3.4) and one level function for By Area and Global. The adapter refuses a bad ladder and a ladder on a strategy other than `contour_parallel`. The engagement radius reads the deepest step (D7). |
| `bf15be13` | The fit rule (`LevelRule::Clip` in `build_material_bool_grid`, with a keep-out mask). Contour parallel on a clip level uses two boundary kinds: the air offset `min(r, stepover/2)` and the link margin (one tool diameter) from the keep-out. It drops eligible parts smaller than one tool diameter. |
| `5ce55f6f` | The fixture sentry `tests/adaptive3d_step_ladder.rs`. |
| `66a2ce8e` | Per-tier counters in the debug trace. `OperationConfig::deepest_axial_step()` goes to the static checks and the narration. |
| `671492e5`, `9771153d`, `ad1bce7c`, `8d5c1d7d` | A GUI test allowance (4 to 2), clippy, a refusal test, and the feeds depth-hint label ("deepest axial step"). |

With an empty ladder the emission is byte-identical. Before the deletion
the parity fixtures were re-blessed on the OLD code with the two dials
off. Only `adaptive.txt` moved (it ran `fine_stepdown` 1.5). The new code
matches all four fixtures with no second bless.

### Acceptance on the fixture

The fixture is a height field 90 × 85 mm and 26 mm deep. It has a deep
flat pocket, a 20° floor into a 60° wall, and two L-shaped pockets joined
by a shelf at Z -12. An independent top-height replay of a flat end mill
reads the emitted path. Runs: By Area, `[10]` with dpp 5, and `[10, 5]`
with dpp 1.

| Claim | Result |
|---|---|
| A1 | The coarse tier cuts 10 mm bites in the open floor. No finer tier removes material there (0.0 mm³). |
| A2 | The drape lifts no clip-tier cut point (lift 0.000 mm). |
| A3 | The largest coarse bite is 10.000 mm (tier 0) and 5.000 mm (tier 1). The base area bite is 1.500 mm at dpp 1. At dpp 5 it is 8.500 mm; see the deviations. |
| A4 | Every clip-tier tool centre is at least 5.77 mm from the keep-out of its level (margin 6 mm, tolerance two cells). The next tier cuts 100 % of its length in closed runs. |
| A5 | At dpp 1 the final stock equals the one-step plan (0.000 mm). At dpp 5, see the deviations. |

Two injected defects make the test fail: the clip rule off (A2, lift
9.1 mm) and a zero link margin (A2, lift 13 mm).

### Deviations from the plan

- A3, base tier: at dpp 5 the base tier bites 8.5 mm (area measure) on the
  60° wall. The one-step plan bites the same 8.5 mm at the same place: the
  drape of a base level on a wall takes the terrace that the level above
  left (F3). This is not a ladder regression. The test bounds the base
  tier by `max(dpp, one-step bite) + one cell`.
- A5 at dpp 5: the plan says "within the leave". The two plans differ by
  up to 2.6 mm on the 60° wall, because their rings put the terraces in
  different places. The test asserts three weaker claims: the difference
  is at most one base terrace; the ladder leaves no more material above
  the leave (9062 mm³ against 9117 mm³); the ladder adds no cut below the
  leave.
- Global runs the waterline cleanup after base-tier levels only. On a clip
  level the mesh contour lies on the wall, where the band still stands to
  the slab top, so a cleanup there is a full coarse-step slot. With an
  empty ladder every level is a base level, so nothing changes.
- F9 is a known limit: By Area still runs the cleanup once, at the end.
  `waterline_cleanup` has no region filter and a region is a bounding box
  (F4). A per-region cleanup needs the region filter of Phase 3 first.
- Entry pecks still step at `depth_per_pass`. A peck is a chip-break
  schedule, not a bite bound.

### Design finding: tier 1 of [10, 5] cuts almost nothing

With `[10, 5]` and dpp 1, tier 1 (5 mm, clip) cuts 32 mm of path and
892 mm³, against 27 715 mm³ for tier 0 and 59 578 mm³ for the base tier.
Cause: both clip tiers erode their eligible area by the same link margin
from the same keep-out. At `5@-10` the keep-out is the keep-out of
`10@-10`, so tier 1 sees the same band that tier 0 left and may not enter
it. The band goes to the 1 mm base tier. On a vertical wall this is true
at every slab; on a sloped wall tier 1 gets only the thin strip where the
keep-out recedes between `-5` and `-10`.

Proposal (not implemented): a graded margin per tier. Clip tier `k` of
`n` clip tiers uses `margin_k = (n - k) × D`. For `[10, 5]` + 1:
tier 0 uses 2 D, tier 1 uses 1 D. Reasoning:

- Each finer tier then has its own band of width D inside the band of the
  tier above, so it can cut one linked closed loop at its own step. The
  bands step in toward the wall like terraces, which is the shape §3.3
  asks for.
- The base tier still receives a band of width D, as today.
- The cost is a smaller coarse area: tier 0 loses one more D next to each
  wall. With one clip tier (`[10]` + 5) the rule gives `1 × D`, which is
  today's behaviour, so the tested case does not change.

An alternative is to give tier 1 no margin at a level that is also a
tier-0 level. It is simpler but it lets tier 1 cut the whole tier-0 band
as slivers at the wall, which §3.3 exists to stop.

### Follow-ups

- feeds readers of `depth_per_pass` as the deepest bite (the other
  session owns them): `feeds/suggest/axial_envelope.rs:280-285` (it reads
  and writes the field; with a ladder it must decide which step it
  clamps), `feeds/predict.rs:371` and `:772`,
  `feeds/operating_point.rs:220`, `feeds/suggest/aggressiveness.rs:198`,
  `feeds/cutter_constraints.rs` through Suggest.
  `tool_load/optimize/axes.rs:216` searches the base step, which is
  probably correct.
- viz feeds readers: `ui/properties/pills.rs:371/503/538` and
  `ui/feeds/compare.rs:57/87`.
- The feeds depth-hint label is done (`8d5c1d7d`).
- Phase 3, region split: in the fixture, the joined pockets C1 and C2 are
  one region (region 1 of 3, 9211 cells). Region detection at each level
  (Phase 3 item 2) is necessary to separate them.
- Phase 4, MCP wire: `params_value_including_nulls`
  (`compute/catalog.rs:1180`) shows an empty `coarse_steps` as `null`, and
  `set_toolpath_param coarse_steps null` does not deserialize. An array
  works. Accept `null` as empty, or write the empty list.
- Phase 5, instrument: `tests/common/zladder.rs` and the ceiling in
  `adaptive3d_commanded_ladder.rs` assume that Z goes down level by level.
  The nested schedule goes back up (-10, -5, …, -10). The instrument
  needs a per-tier ladder before it can measure Arm C.
- D5: decide `[10, 5]` against `[10]` + 5 after the margin fix.

### Tests after the rebase onto `5da59417`

- `adaptive3d_step_ladder` 7/7, `adaptive3d_emission_byte_parity` 5/5,
  `adaptive3d_commanded_ladder` 1/1.
- Heavy, run by name: `adaptive3d_interior_cell_parity_f029` 2/2 (54 s),
  `adaptive3d_planner_stock_xy_f027` 2/2 (49 s).
- `rs_cam_core --lib diagnostics` 47/47; `rs_cam_cli` 57/57.
- Before the rebase: the five folder sentries, `--lib` adaptive3d /
  catalog / spans / annotate / execute / diagnostics 178/178,
  `session::compute` 70/70, `rs_cam_viz` 1011/1011.
- The full clippy line and `fmt --check` are clean.
- rivmap100 parses: 4 toolpaths; the 3D Rough has dpp 2 and an empty
  ladder.

## Arm C preview on rivmap100 (master b1a8fda5, before Phase 2b)

One instrument: `rough-score --toolpath 1 --resolution 0.5`, By Area, the
project's machine (accel time in `cut_summary.total_runtime_s`).

| arm | moves | total s | cutting s | entry s | rapid s | removed mm³ | peak DOC mm |
|---|---|---|---|---|---|---|---|
| 2 only | 8 905 | 1379 | 637 | 538 | 199 | 49 088 | 2.0 |
| [8] > 2 | 9 405 | 1396 | 630 | 552 | 208 | 49 096 | 6.0 |
| 4 only | 6 740 | 1045 | 507 | 398 | 137 | 48 648 | 4.0 |
| [8] > 4 | 7 148 | 1055 | 505 | 405 | 141 | 48 673 | 6.0 |
| 5 only | 5 373 | 814 | 431 | 271 | 111 | 47 299 | 5.07 |
| 8 only | 3 175 | 472 | 256 | 156 | 59 | 43 175 | 6.0 |
| 10 only | 3 922 | 586 | 268 | 242 | 76 | 47 007 | 8.0 |

- On rivmap100 the ladder gives NO time gain: [8] > 2 equals 2 only, and
  [8] > 4 equals 4 only (+1 %). The pockets are narrow and intricate; after
  the one-diameter erosion almost no area fits the coarse tier. The
  pre-registered bound (Arm C between B10 and B5) is therefore not met in the
  useful direction: the ladder takes the base step's time.
- The time on this model comes from a larger BASE step. Entry time is 36–40 %
  of every arm.
- "8 only" is fastest but removes 4 000 mm³ less than "10 only"; its bottom
  level lands differently. Check the finish before choosing it.
- The CLI `--set` coercer cannot parse an array (`--set 1.coarse_steps=[]`
  is refused: "invalid type: string \"[]\""). Phase 4 follow-up.

## Demo finding (operator, 2026-09-24): "it only did one pocket"

The operator watched `[8] > 2` on rivmap100 and saw one pocket get a
coarse pass; the rest cut like before. The arithmetic explains it:

- G-LADDERANCHOR: the ladder starts at the stock BOX top
  (`stock_top_z: ctx.stock_bbox.max.z`, `compute/execute/finish_3d.rs:194`),
  Z 14 here (origin −11, height 25), not at the stock the operation reads
  (the Face took it to Z 12). So the one coarse level lands at Z 6.
- The fit rule then admits only floors below Z 5.5. Most rivmap100 floors
  are at Z 2–4 but many pockets are shallower, so one pocket qualified. The
  simulation's peak DOC 6.0 mm (12 → 6) confirms the coarse level cut there
  only.
- Quantisation (design, but against the operator's words): a pocket
  shallower than `step + leave` below the slab top never gets a coarse pass,
  even when its whole depth is less than the coarse step. The operator's
  intent was "if a region fits into 10 mm, cut it in one".

Fix candidates: (1) anchor the ladder at the seed stock's top when the
operation reads prior stock; (2) a fit rule that lets a pocket whose whole
remaining depth is less than the coarse step take one coarse pass to its
floor, with finer steps only on the steep wall band.
