# Phase 4 — compact spiral refusal count on the wanaka small-tool regions

Gate G3, step 1 of `PLAN.md` "Phase 4 — spiral finish for the many-island
case". This note records a count. It does not rank or score.

## Header

- Code: worktree `.claude/worktrees/adaptive-head`, HEAD
  `df6de926000539f2438ac6791583a5701019775a` plus the uncommitted
  instrument `crates/rs_cam_core/tests/spiral_finish_compact_wanaka_g3.rs`
  and its `[[test]]` row in `crates/rs_cam_core/Cargo.toml`.
- Command (from the worktree):
  `CARGO_TARGET_DIR=$PWD/target timeout 240 scripts/cargo_lane.sh test --release -p rs_cam_core --features research,test-support --test spiral_finish_compact_wanaka_g3 -- --ignored --nocapture`
- Wall clock: 7.1 s in the test (tier map 5.5 s, level sets and bridge
  0.4 s). Three runs gave the same result for every island (8.0 s, 6.6 s,
  7.1 s).
- Fixture: `planning/multitool_2026-08-23/wanaka200_mt1.toml`, model 0
  (`/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl`), setup 1
  ("Setup 2 — front").
- Ladder: tools 0–4, that is the 6 mm end mill and the R2.0, R1.5, R1.0 and
  R0.5 tapered balls. The planner sorts them coarse to fine by cusp radius.
  The V-bit (tool 5) is an engraving tool and is not on the ladder.
- Dials: `MultitoolPlanSpec::default()`. Cell 0.4 mm, tolerance 0.05 mm,
  margin 0.5 mm, `SlopeCompensated`, overlap 2.0 mm, cap 24 regions per
  tier, coarseness 1, cusp 0.03 mm. The run did not need a coarser cell.
- Path: `ProjectSession::preview_multitool_plan`, the planner's own path.
  The instrument counts every `owned` polygon of tiers 1–4.

## Deviations from the brief

1. Level sets. C1 made its level sets with
   `direction_field::solve_paths_with_target` and a closed-form EDT. That
   EDT exists only on a disk. This instrument takes the iso-distance
   offsets directly instead. It rasterises each island at stepover / 4
   (the F2 grid rule), takes `distance_transform_2d`, and traces the
   boundary of `{EDT >= k * stepover}`, k = 0, 1, … with
   `marching_squares_bool_grid`. Level 0 is the island outline. The module
   doc defines its input as these offsets. Every refusal variant tests loop
   topology and shape, not spacing. Bridge params are the C1 params
   (`hub_cover_reach_mm` = lateral reach, near-hub span = `PAPER_INITIAL_BRIDGE_SHIFT`).
   Controls run first: a disk of radius 6 mm and one of 3 mm, through the
   same pipeline, at each of the four cusp radii. All eight controls return
   `Ok` (5 to 18 rings).
   The lead can overrule this substitution.
2. Project load. The loader refuses `wanaka200_mt1.toml`: the Drill
   toolpath carries the retired key
   `setups[0].toolpaths[2].operation.params.depth`. The instrument removes
   the nine `[[setups.toolpaths]]` sections in memory and parses the
   rest. The fixture file is not written. The tier map does not read the
   toolpaths.

## Per tier

| tier | tool | stepover mm | islands | area mm² | Ok | refusals by variant |
|---|---|---|---|---|---|---|
| 1 | R2.0 tapered ball | 0.690 | 1 | 31 355 | 0 | LevelNotSingleLoop 1 |
| 2 | R1.5 tapered ball | 0.597 | 7 | 708 | 0 | LevelNotSingleLoop 5, NotStarShaped 2 |
| 3 | R1.0 tapered ball | 0.486 | 15 | 428 | 0 | LevelNotSingleLoop 15 |
| 4 | R0.5 tapered ball | 0.341 | 24 | 2 888 | 0 | LevelNotSingleLoop 24 |
| all | | | 47 | | **0** | LevelNotSingleLoop 45, NotStarShaped 2 |

The island count equals the `owned` polygon count in every tier. The cap
acted on tier 4 only (24 islands kept from 4 297 raw). Tier 1 is one
polygon with 1 293 holes over most of the board. It is not a small
region, but the brief includes it.

## Per island

`holes` is the number of holes of the owned polygon, and `largest hole` is
the area of its largest hole. `levels` is the number of non-empty level
sets that the bridge received. One planning cell is 0.16 mm².

| tier | island | area mm2 | holes | largest hole mm2 | levels | result |
|---|---|---|---|---|---|---|
| 1 | 0 | 31355.2 | 1293 | 1272.56 | 34 | LevelNotSingleLoop { level: 0, loops: 1294 } (grid 1025x1025 @ 0.195 mm) |
| 2 | 0 | 215.3 | 2 | 3.76 | 4 | LevelNotSingleLoop { level: 0, loops: 3 } (grid 59x552 @ 0.149 mm) |
| 2 | 1 | 146.3 | 6 | 6.80 | 3 | LevelNotSingleLoop { level: 0, loops: 7 } (grid 244x193 @ 0.149 mm) |
| 2 | 2 | 104.5 | 1 | 1.36 | 4 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 362x48 @ 0.149 mm) |
| 2 | 3 | 98.6 | 2 | 6.80 | 3 | LevelNotSingleLoop { level: 0, loops: 3 } (grid 215x233 @ 0.149 mm) |
| 2 | 4 | 63.9 | 0 | 0.00 | 2 | NotStarShaped { level: 0, backtrack_rad: 0.36505526650195197 } (grid 469x587 @ 0.149 mm) |
| 2 | 5 | 43.0 | 1 | 2.80 | 3 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 148x48 @ 0.149 mm) |
| 2 | 6 | 36.6 | 0 | 0.00 | 3 | NotStarShaped { level: 0, backtrack_rad: 0.3521264145410551 } (grid 220x40 @ 0.149 mm) |
| 3 | 0 | 97.4 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 2 } (grid 1014x839 @ 0.122 mm) |
| 3 | 1 | 35.0 | 1 | 0.72 | 3 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 83x194 @ 0.122 mm) |
| 3 | 2 | 32.3 | 1 | 0.72 | 2 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 112x191 @ 0.122 mm) |
| 3 | 3 | 28.8 | 1 | 0.72 | 3 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 79x142 @ 0.122 mm) |
| 3 | 4 | 28.2 | 1 | 1.52 | 2 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 119x132 @ 0.122 mm) |
| 3 | 5 | 27.8 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 8 } (grid 76x135 @ 0.122 mm) |
| 3 | 6 | 27.1 | 2 | 0.88 | 2 | LevelNotSingleLoop { level: 0, loops: 3 } (grid 79x188 @ 0.122 mm) |
| 3 | 7 | 25.0 | 7 | 0.72 | 2 | LevelNotSingleLoop { level: 0, loops: 8 } (grid 96x145 @ 0.122 mm) |
| 3 | 8 | 20.4 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 5 } (grid 89x162 @ 0.122 mm) |
| 3 | 9 | 20.1 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 4 } (grid 106x96 @ 0.122 mm) |
| 3 | 10 | 17.4 | 1 | 2.96 | 2 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 89x83 @ 0.122 mm) |
| 3 | 11 | 17.2 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 2 } (grid 106x79 @ 0.122 mm) |
| 3 | 12 | 17.1 | 1 | 1.20 | 3 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 63x89 @ 0.122 mm) |
| 3 | 13 | 17.0 | 1 | 2.16 | 2 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 69x89 @ 0.122 mm) |
| 3 | 14 | 16.9 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 6 } (grid 112x116 @ 0.122 mm) |
| 4 | 0 | 383.6 | 20 | 155.92 | 3 | LevelNotSingleLoop { level: 0, loops: 21 } (grid 731x745 @ 0.085 mm) |
| 4 | 1 | 322.0 | 20 | 70.64 | 3 | LevelNotSingleLoop { level: 0, loops: 21 } (grid 440x890 @ 0.085 mm) |
| 4 | 2 | 288.6 | 13 | 5.84 | 3 | LevelNotSingleLoop { level: 0, loops: 14 } (grid 628x698 @ 0.085 mm) |
| 4 | 3 | 272.6 | 8 | 28.08 | 3 | LevelNotSingleLoop { level: 0, loops: 9 } (grid 731x778 @ 0.085 mm) |
| 4 | 4 | 250.8 | 20 | 5.36 | 3 | LevelNotSingleLoop { level: 0, loops: 21 } (grid 633x651 @ 0.085 mm) |
| 4 | 5 | 164.9 | 10 | 0.88 | 2 | LevelNotSingleLoop { level: 0, loops: 11 } (grid 525x562 @ 0.085 mm) |
| 4 | 6 | 159.8 | 8 | 5.04 | 3 | LevelNotSingleLoop { level: 0, loops: 9 } (grid 323x862 @ 0.085 mm) |
| 4 | 7 | 129.7 | 4 | 5.36 | 2 | LevelNotSingleLoop { level: 0, loops: 5 } (grid 422x356 @ 0.085 mm) |
| 4 | 8 | 105.6 | 1 | 3.12 | 2 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 454x295 @ 0.085 mm) |
| 4 | 9 | 96.7 | 4 | 7.44 | 2 | LevelNotSingleLoop { level: 0, loops: 5 } (grid 271x497 @ 0.085 mm) |
| 4 | 10 | 85.0 | 2 | 0.56 | 2 | LevelNotSingleLoop { level: 0, loops: 3 } (grid 234x436 @ 0.085 mm) |
| 4 | 11 | 81.8 | 5 | 104.08 | 2 | LevelNotSingleLoop { level: 0, loops: 6 } (grid 234x712 @ 0.085 mm) |
| 4 | 12 | 76.6 | 4 | 2.32 | 3 | LevelNotSingleLoop { level: 0, loops: 5 } (grid 290x328 @ 0.085 mm) |
| 4 | 13 | 69.4 | 3 | 5.36 | 2 | LevelNotSingleLoop { level: 0, loops: 4 } (grid 290x365 @ 0.085 mm) |
| 4 | 14 | 58.4 | 1 | 0.56 | 2 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 253x379 @ 0.085 mm) |
| 4 | 15 | 49.8 | 3 | 0.72 | 2 | LevelNotSingleLoop { level: 0, loops: 4 } (grid 187x361 @ 0.085 mm) |
| 4 | 16 | 45.8 | 4 | 5.36 | 2 | LevelNotSingleLoop { level: 0, loops: 5 } (grid 131x295 @ 0.085 mm) |
| 4 | 17 | 40.1 | 2 | 0.40 | 2 | LevelNotSingleLoop { level: 0, loops: 3 } (grid 225x150 @ 0.085 mm) |
| 4 | 18 | 38.3 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 22 } (grid 295x215 @ 0.085 mm) |
| 4 | 19 | 39.0 | 3 | 1.36 | 3 | LevelNotSingleLoop { level: 0, loops: 4 } (grid 220x234 @ 0.085 mm) |
| 4 | 20 | 34.6 | 1 | 3.28 | 2 | LevelNotSingleLoop { level: 0, loops: 2 } (grid 361x154 @ 0.085 mm) |
| 4 | 21 | 34.6 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 10 } (grid 337x407 @ 0.085 mm) |
| 4 | 22 | 30.5 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 15 } (grid 150x295 @ 0.085 mm) |
| 4 | 23 | 29.8 | 0 | 0.00 | 2 | LevelNotSingleLoop { level: 1, loops: 14 } (grid 281x192 @ 0.085 mm) |

## Reading

The module refuses every small-tool region on this fixture. It accepts 0
of 47 islands. It accepts all eight control disks, so the level-set
pipeline does not cause the 45 `LevelNotSingleLoop` refusals: a grid
staircase cannot split one loop into two. 35 islands have holes, and a
polygon with a hole gives two or more loops at level 0. The module refuses
that shape by design. Most of these holes are small: in tiers 2 and 3 the
largest hole is 0.7 to 6.8 mm², that is 4 to 43 planning cells. The island
filter (`min_region_area_mm2`) removes small islands but does not fill
small holes. This run does not measure the count with the holes filled.
The 12 islands without holes refuse also. Ten split into 2 to 22 loops at
the first inner offset (level 1), which is the branched signature. Two
(tier 2, islands 4 and 6) are not star-shaped at the outline. Their
backtrack is 0.365 and 0.352 rad against the 0.35 rad limit. For these
two, the grid staircase can be the cause, and the run does not exclude
it. If the module accepted both, the count would be 2 of 47. Most regions
refuse. `PLAN.md` Phase 4 step 2 applies: record the result and stop the
compact spiral arm.
