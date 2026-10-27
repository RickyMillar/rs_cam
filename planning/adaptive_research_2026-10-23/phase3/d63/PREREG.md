# D63 pre-registration (G-RESTDOC anchor fix), written before the change and before any run

Base: `f2724a5bd` (adaptive-head HEAD).

## The change

1. One shared cell test, `tool_reach_cells`: a planner cell is in the tool's reach when its square touches a disc of the tool radius centred inside the op's centre containment (`Adaptive3dGeometry::centre_boundary`). This is the sliver-safe test of the entry-floor read: the cell centre lies within R + cs/2 of the containment, built as the containment grown by R + cs/2. An empty containment means every cell.
2. One snapshot, `PriorStockTops`: the column top of each reach cell, read from the planner stock AFTER the border clear and BEFORE the boundary clear. It exists only on prior stock (`initial_stock` set). On fresh stock nothing is read, as before.
3. `level_anchor_z` (G-LADDERANCHOR) and `rest_ceiling_keeps_doc` (the D50 guard) read only this snapshot:
   - the anchor is the highest top over reach cells with stock above `surface + stock_to_leave`;
   - the guard asks whether any reach cell holds more than one Depth/Pass above THE level floor of the first level.
   No new number.

## Which outputs can change

- Fresh-stock ops: none (the anchor is `stock_top_z`; the guard is not reached). All byte-parity fixtures are fresh-stock 3D Roughs, so they stay byte-identical.
- 3D Roughs on remaining stock (`from_remaining_stock`):
  - With no boundary: the reach is every cell, and the snapshot equals today's stock (no boundary clear runs). No change.
  - With a boundary: the anchor now also reads cells outside the raw boundary but within the tool's reach. It changes only where the previous ops left stock there that stands higher than everything inside the raw boundary. With `Center` containment that is the band one tool radius beyond the raw line (G-RESTDOC). With `Inside` it is at most the half-cell row at the raw line.
  - It can also drop where today's no-boundary read takes stock that the tool can never reach (a multi-region `DerivedRestRegions` consumer reads every cell today; wanaka op 7 reads its frame). Its anchor can then fall to the highest stock within reach.
  - The D50 guard reads reach cells, not containment-interior cells, so it refuses the ceiling in more cases. That only ever keeps a higher start.
- The op golden (`every_op_keeps_its_toolpath_g_opgolden`): expected unchanged, unless one of its ops is a bounded rest 3D Rough with such stock. If any row moves, stop and report it.

## Expected numbers

- Corne B s2.1, op 5 after the geodesic op 1 (D61 setup, R `dabf258d3`/`f2724a5bd` jobs): the anchor rises from 20.04 to 23.0, the stock top of the band. The D50 ceiling (17.0) is refused, because the band then puts 12 mm on a first level at 11. So the first level is 17.0 = 23 − 6.0, and the levels may become 17 / 11 / 5 / 0.5 (count 4 or 5).
- The max axial cut of op 5 is at most one DPP (6.0) plus the drape. D61's 8.96 mm at Z 14.04 goes.
- Sentry planted case: a CP rest op after a source that leaves a full-height band within tool reach beyond its raw line. The first level is at most one DPP below the band top, and no sim sample cuts deeper than one DPP plus the drape.
- The D62 planted case (CP op 1 offset 0, CP rest op offset 6): first level 17.0, unchanged.
