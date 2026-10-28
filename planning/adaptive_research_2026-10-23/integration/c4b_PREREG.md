# C4b, "Contour each level" in By Area order (the F9 fix): pre-registration

Date: 2026-10-28. Base: `geodesic-integration` `e8072f172` (C4). Work in an
isolated worktree with its own lane script and target directory. Ruling:
OPERATOR_DECISIONS_2026-10-28.md, D69: the option covers the walls; the
lead (C4 approval): with the option on, By Area runs the wall cleanup per
region-level with a cell mask; with the option off, By Area is
byte-identical.

## 1. Today

- Global order: `waterline_cleanup` runs after every level (walls steeper
  than 30°, every 3D Rough). C4b does not touch Global.
- By Area: the cleanup runs ONCE, at the bottom Z, after every region
  (`adaptive3d/path.rs`, the F9 comment). Reason (F9): the cleanup traces
  every mesh contour at the Z and has no cell mask, so a per-region run
  would cut the walls of regions not roughed yet, through their full
  stock.

## 2. The change (option on only)

In By Area order, after each `clear_planned_level` of a region (and the C4
band pass), the wall cleanup runs at that level with a cell mask. The final
bottom run stays as today (unmasked, after every region: safe today, and it
cleans the wall points no region-level run could take, see 3).

`waterline_cleanup` gains one argument, `keep: Option<&dyn Fn(P3) -> bool>`.
`None` (Global, the By Area bottom run, the option off) is today's code
path. `Some`: each traced contour (after the existing steep and left-piece
filters) is split into the runs of points that `keep` accepts. A fully kept
closed contour is emitted as today (one Rapid, one closed Cut). Each
partial run is one Rapid and one open Cut through the same
`push_segment_with_stamp` door (entry through `plan_entry`, stamp in the
planner stock).

## 3. The mask rule (no typed number)

A cleanup point `p` at level `z` in region `k` is kept when NO planner cell
in the tool's reach of `p` (centre within R + cs/2, the cell reach of the
stamp; the reach C4 reads, `PriorStockTops`) holds planner stock above
`z + MATERIAL_MARGIN_MM` and lies outside region `k`'s mask
(`AreaMask::owned`).

- `MATERIAL_MARGIN_MM` is the existing 0.01 mm margin of the level anchor
  (`path::level_anchor_z`, `level_contour.rs`), not a new number.
- Cells of region `k` may hold stock above `z`: that is the wall stock the
  cleanup exists to cut, inside the region being roughed.
- Cells of another region count only when their stock still stands above
  `z`. A region already roughed to `z` or below does not block, and a cell
  with no material does not block.
- A point whose disc meets two regions' unroughed stock is not cut at that
  region-level. The final bottom run, after every region is roughed (as
  today), takes it.

How F9 stays impossible: F9's cut is a cleanup point whose tool disc meets
stock of a region that is not roughed yet, above the cleanup Z. The rule
keeps no such point: every cell in reach that holds stock above `z` is in
the current region's mask. The stock of other regions above `z` is never
touched by a region-level cleanup cut.

## 4. Sentries

1. Option off: `adaptive3d_emission_byte_parity` case
   `contour_parallel_by_area` and the op-golden row
   `adaptive3d/by_area_helix_cp` unchanged; every other case and row
   unchanged.
2. `by_area_walls_cut_each_region_level_c4b` (session): a fixture with two
   regions (two pockets in one block, so each region's walls stand at
   every level), By Area, option on:
   - non-vacuity: region-level cleanup cuts exist at a level of region 1
     before region 2 starts (`WaterlineCleanup` markers between the
     region markers);
   - F9 safety: the simulated axial engagement of the op is at most
     DPP + one sim cell (the cleanup never cuts the other region's full
     stock).
   - Planted red: `keep` accepts every point. Then the region-1 cleanup
     cuts region 2's walls through its full stock, and the axial check
     fails (F9 reproduced).
3. The mask rule (unit, `adaptive3d`): a point whose reach meets a cell of
   another region with stock above `z` is rejected; the same cell cut to
   `z` is accepted; a cell of the own region with stock is accepted.
4. Determinism: two option-on By Area runs in one process give the same
   moves.

## 5. Predicted re-bless list

- **Option off: none.** By Area takes today's path (`keep` is `None`).
  Global is unchanged with the option on or off.
- Option on: no shipped fixture sets it with By Area. No re-bless.
- One doc change: C4's catalog sentence "A fresh rough is unchanged" holds
  for Global order only. With the option on, a fresh By Area rough gains
  the region-level wall cuts. I amend the sentence, and C4's
  `a_fresh_rough_ignores_contour_each_level_c4` (Global) stays as it is.

## 6. Size

About 120 lines (`waterline_cleanup` keep argument and run split, the mask
closure in the By Area loop), 2 test files. Stop points: the `keep`
argument with `None` everywhere (gates unchanged), then the By Area wiring
and sentries.
