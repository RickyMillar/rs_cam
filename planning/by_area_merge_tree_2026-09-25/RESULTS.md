# By Area pocket tree — measurement (2026-09-25)

Status: MEASURED (2026-09-25). Since WP2 (2026-10-01, working tree) the
By Area planner reads the tree; see "Phase 3 measurement" at the end.

## Question

By Area finds one region on rivmap100, so it cuts as Global does. Can a
join (merge) tree of the tool-CL height grid with a persistence threshold
find the real valleys? The operator expects "fewer than 10 pockets".

## Method

- `crates/rs_cam_core/src/surface/merge_tree.rs`: `build_pocket_tree`.
  Cells go in by (Z, index). Union-find joins basins at their saddle. A
  basin that is less than `persistence_h_mm` deep below its saddle, or
  smaller than `min_area_mm2`, merges into its parent. References: Carr,
  Snoeyink & Axen 2003; Grimaud 1992; Soille 2003 (CREDITS.md).
- The grid border and masked cells are walls, not outlets. A valley that
  runs off the board edge is a pocket. The priority flood of
  `flow_accum` drains such a valley, so the tree does not use it.
- Input: the drop-cutter grid of the 3D Rough tool, from the planner's own
  builder. Cell 0.5 mm, 221 x 221. Material top Z 12.0 (after the Face).
  8440 cells outside the mesh are masked.
- Probe: `crates/rs_cam_core/tests/pocket_merge_tree_census_by_area.rs`
  (ignored). The probe takes 19 s.

## Results

Material area 8760 mm². A leaf pocket is a real valley. An internal pocket
is the band between a valley's saddle and the next level. The root is the
ground above Z 7.585 (4427 mm², 11.8 mm deep).

| h (mm) | min area (mm²) | pockets | leaves | leaf area (mm²) | leaf depths (mm) |
|---|---|---|---|---|---|
| 1 | 100 | 11 | 6 | 2842 | 1.41–7.38 |
| 1 | 400 | 5 | 3 | 3878 | 2.86–7.38 |
| 1 | 1600 | 3 | 2 | 4334 | 5.95–7.38 |
| 2 | 100 | 9 | 5 | 2708 | 2.34–7.38 |
| 2 | 400 | 5 | 3 | 3878 | 2.86–7.38 |
| 2 | 1600 | 3 | 2 | 4334 | 5.95–7.38 |
| 4 | any | 3 | 2 | 4334 | 5.95–7.38 |
| 8 | any | 1 | 1 (the root; no split) | – | – |
| 2 (0.25 mm cell) | 400 | 5 | 3 | 3857 | 2.86–7.34 |

The current detector finds 1 region (8703 mm², box 3.25–96.75). It
flood-fills the material that remains, so it cannot find a basin.

- At h ≤ 2 mm, the minimum area sets the count. At h ≥ 4 mm, h sets it.
- Half the cell size gives the same pockets (leaf area −0.5 %).
- Build time: height grid 143 ms (release, from the planner log); tree
  25 ms at 0.5 mm.

Images: `images/pockets_h2_a400_ids.png` (3 valleys: NW 7.4 mm, S 5.7 mm,
E 2.9 mm), `images/pockets_h1_a100_ids.png` (6 valleys),
`images/pockets_h4_a400_ids.png`, `images/pockets_h2_a400_order.png`,
`images/current_by_area_regions.png`.

## Findings for Phase 3

1. The pocket count is not the valley count. Only the leaves are valleys.
   The internal bands are thin. A band cut as its own job splits the
   rough into small pieces.
2. About half the material is the root (the high ground). By Area gains
   only on the leaves.
3. The planner cuts a By Area region by its bounding box, not by its
   cells (see `inspect_spans` `area_regions.note`). Leaf valleys are not
   convex and their boxes overlap. Phase 3 must clip each level to the
   pocket's cells, or the boxes cut other pockets' material.
4. The "children first" order is not global deepest-first. A sibling
   order rule must be chosen.

`build_pocket_tree` is `pub` with no product reader yet. The probe and
Phase 3 are its readers. A dead-pub sweep must not delete it.

Proposed default for Phase 3: h = 2 mm, min area = 400 mm² (3 valleys on
rivmap100). The operator has not chosen a setting yet.

## Phase 3 measurement (WP2 + WP4 + WP5, 2026-10-01)

Status: MEASURED, working tree on master a0f75b27 (uncommitted). WP2 is in
the product: By Area builds the pocket tree in the planner
(`adaptive3d/area_plan.rs`). WP3 (the dials as settings) and WP6 (delete
the slower order) are not done. **Verdict: By Area is SLOWER than Global on
rivmap100 in both orders. Per PLAN §7.5 the work stops here for an
operator decision.** The earlier "By Area 728-813 s, one region" numbers
came from the flood-fill detector, which found no pocket. They are not
evidence for the tree and are not compared here.

### What WP2 changed

- The tree reads the planner's own tool-CL grid (`max(CL, z_floor)`;
  uncovered or out-of-boundary cells masked) under `top_z = level_top`.
- Dials (core consts until WP3): minimum depth 1/3 x D (operator ruling
  2026-09-26), minimum area 400 / (π · 3²) = 14.147 tool discs (gives the
  proposed 400 mm² at Ø6; awaits operator confirmation).
- Jobs: one per valley (leaf; a childless root counts), one rest job. A
  tree with one pocket has no valley: the rest runs with Global's levels
  and per-level cleanup, so By Area emits Global's moves (sentry S1).
- Ownership per level: rest first, valley L owns its cells below its
  saddle, the rest owns everything else. Rest last: valleys own their
  cells at every level, the rest owns every cell. Valleys go nearest next.

### Terrain census (the operator's question: a handful of catchments?)

Probe: `tests/pocket_merge_tree_census_by_area.rs` (ignored), now with
`MERGE_TREE_ROUGH`, `MERGE_TREE_SET`, a raw run (h 0, area 0) and a run at
the planner dials. CSVs: `wp2_census/`. Images: `images/wp2_*.png` (one
colour per pocket over the shaded CL surface; grey = masked).

| Terrain (op, tool) | raw pockets (leaves) | at the dials: pockets / valleys | valley depth, area, centroid XY | planner jobs |
|---|---|---|---|---|
| rivmap100 (3D Rough, Ø6, dpp 8) | 445 (223) | 5 / 3 | NW 7.38 mm, 1992 mm², (29.2, 70.8); S 5.69 mm, 1154 mm², (55.7, 25.2); E 2.86 mm, 731 mm², (84.5, 56.0) | rest (4826 mm²), S, E, NW |
| wanaka100 (3D Rough 6, Ø6) | 623 (312) | 1 / 0 | none | rest only (= Global) |

- rivmap100: three catchments, the same three as the 2026-09-25 probe.
  `images/wp2_rivmap100_planner_jobs.png` is the planner's own map.
- wanaka100 is a FINDING: 0 valleys. The CL relief of the terrain face is
  about 5.6 mm (CL -1.67 .. 3.95 mm in the model frame). At h 1 mm the
  tree has two leaves: a SW basin 3.71 mm deep (5821 mm², centroid
  (37.5, 34.7)) and a NE basin 1.56 mm deep (1743 mm², (77.1, 81.9)),
  joined at Z 2.04 (`images/wp2_wanaka_pockets_h1_a400.png`). At the dial
  h = 2 mm the NE basin is weak (1.56 < 2), so it joins the SW basin and
  the tree has one pocket. The cause is the merge rule together with the
  dial: a basin becomes a valley only when it meets ANOTHER significant
  basin, so one deep basin on a plateau is never split from the high
  ground. The flow-field build is not the cause (the planner map and the
  model-frame probe agree on 1 pocket). The dials were not tuned.
- rivmap350: no fixture branch appeared during this work; not measured.

### WP5 arms (rivmap100_live_0925.toml, dpp 8, 0.5 mm, release CLI)

`rs_cam_cli rough-score <F> --resolution 0.5 --set 1.depth_per_pass=8
--set 1.region_ordering=<global|by_area>`; A-last is the same build with
`AREA_ORDER = RestLast` (a temporary const, restored after). Global on the
A-last build gives the same 844.5 s, so the builds are comparable. Toolpath
1 (the 3D Rough) only: the Scallop Finish is disabled in this project, so
the finish-equivalence check (PLAN §7.2) was NOT measured. A-h1 / A-h4
need the WP3 dials and were not run.

| Arm | total s | cutting s | entry s | rapid s | rapid mm | cut mm | removed mm³ | wce | avg eng | entries | rapid moves | moves | rapid collisions | peak DOC | planner s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| G (Global) | 844.5 | 748.4 | 423.8 | 96.1 | 4912 | 18530 | 48639 | 0.1297 | 0.1463 | 116 | 219 | 9499 | 0 | 8.0 | 1.04 |
| A (rest first) | 1031.8 | 890.6 | 455.8 | 141.2 | 6618 | 20016 | 49238 | 0.1194 | 0.1383 | 189 | 354 | 11546 | 0 | 8.0 | 1.31 |
| A-last | 1453.5 | 1127.6 | 767.6 | 325.8 | 18249 | 24699 | 48247 | 0.0668 | 0.0861 | 290 | 747 | 15530 | 0 | 8.0 | 1.32 |

Planner s = log time from "Z levels computed" to "3D adaptive toolpath
complete" (tree build 74-87 ms of it).

Against PLAN §7.3, for A (the faster order):

- F1 FAILS: 1031.8 s >= 844.5 s (+187.3 s, +22.2 %). A-last: +72.1 %.
- F2 FAILS: whole-cycle engagement 0.1194 < 0.1297.
- F3 not measured (finish disabled). Rough volume: A 101.2 % of G (pass);
  A-last 99.19 % (< 99.5 %, fails).
- F4: peak axial DOC 8.0 in every arm; deflection findings not read.
- F5 passes: 3 valleys.
- F6 not measured (no GUI run).
- F7 FAILS: planner +26 % (> 20 %).
- Rapid collisions: 0 in every arm.

Confound (PLAN §7.4): Global runs the waterline cleanup after each of its
2 levels; rest first runs it once at the bottom.

Where the time goes (aggregates only; not yet measured per job): A has
+73 entries, +1486 mm of cut and +1706 mm of rapid against G. Hypothesis:
each job boundary (valley rim against the rest) is a wall that the
contour-parallel rings of BOTH jobs trace, and each job is entered
separately, so the masks add perimeter passes and entries that Global does
not have. At dpp 8 there are only 2 levels, so a valley job has 1-2 levels
and gains little depth-first travel to pay for that. Rest last adds a full
Global pass over the remaining stock, with its own entries, after the
valleys.

Next (operator / lead decision, not started): stop By Area here, or
measure the per-job boundary length and entries, or measure at a smaller
dpp before any change.
