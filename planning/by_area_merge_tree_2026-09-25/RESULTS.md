# By Area pocket tree — measurement (2026-09-25)

Status: MEASURED. Research only; the module is on master (6f52d7a3) and no
product path reads it. Phase 3 (By Area uses the tree) is not started.

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
| 8 | any | 1 | 0 | – | – |
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

Proposed default for Phase 3: h = 2 mm, min area = 400 mm² (3 valleys on
rivmap100). The operator has not chosen a setting yet.
