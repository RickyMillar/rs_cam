# D63: where the 14.04 anchor comes from (G-RESTDOC), read only

Tree read: R `dabf258d3` (adaptive-head, the product's op 5). No code changed. All line numbers are at `dabf258d3`.

## 1. 14.04 is the G-LADDERANCHOR anchor 20.04 minus one Depth/Pass

The fix42 traces (the "Compute Z levels" span, `level_top` counter):

| run | op | `level_top` | first level | count |
|---|---|---|---|---|
| dabf258d3 Corne B s2.1 | op 1 (geodesic) | 23.0 | 17.0 | 4 |
| dabf258d3 Corne B s2.1 | op 5 (CP rest) | 20.0415 | 14.0415 | 4 |
| 607e52730 Corne B s2.1 | op 5 (CP rest) | 17.0 | 11.0 | 3 |

- On `dabf258d3`, `level_top` is `level_anchor_z` (`adaptive3d/path.rs:323`, called at `:1024`). The D50 ceiling (`rest_capped_level_top`, `:1035`, top 17.0) was refused by the addendum guard, so the anchor stayed.
- 20.0415 is not an op-1 level. It is an off-grid Z: the highest stock top that op 5's planner still holds (a draped surface plus the leave).
- On `607e52730` (no guard) the D50 ceiling set 17.0, so the first level was 11.0. That is D61's 12 mm pass.
- In both trees the band at 23 is not in what the anchor reads.

## 2. Why the band at 23 does not count

`level_anchor_z` reads every planner cell with stock above `surface + stock_to_leave`. Its doc comment says so: "the border and the boundary clears run first, so a cell outside the mesh footprint or outside the machining boundary does not count" (`:314-317`).

**(a) The geodesic op 1 leaves the band beyond its raw boundary.**
- Op 1 (`Center`, silhouette + 6) is the research arm with Global ordering. Its boundary clear keeps the stock that a disc centred inside the raw boundary reaches (G-CLIPAIR, `boundary_reach`, `research_outer_band.rs:148`, called at `path.rs:953`).
- `ResearchClip::restrict_regions` makes no region there ("§16: the band stock the reach keeps is read by the plan, but no region is made to cut it; the edge contour pass cuts it", `research_outer_band.rs:175`; module doc `:7`, "the stock-edge band go[es] to a separate contour pass").
- The 2D planner insets each region by R (`planner_outlines`). So op 1's discs end at the raw line, and the band from raw to raw + R (6–9 mm outside the silhouette) stays at the stock top, 23.
- A CP op 1 plans its centres up to the raw line (D62's planted case: with offset 0 it cut to 3 mm outside the wall). With offset 6 it cuts out to 9 mm and leaves no band. That is why D62 sees no breach.

**(b) The rest op's boundary clear removes that band from its own planner stock.**
- Op 5 is CP (not the research arm), so its boundary clear (`path.rs:947`) keeps only the cells whose centre is inside the raw boundary (`:965`, `inside = boundary.contains_point`).
- Its raw boundary is the silhouette + 6 in D41, or its derived region in D60. The derived region becomes a single pre-boundary when the region set collapses to one polygon (`session/compute/generation.rs:393`); see the D10 check below.
- With `Center` containment that raw line is the CENTRE limit, so op 5's disc reaches R beyond it: into the band.
- The clear sets the band cells to the surface (`:1004` area). So the anchor never sees the band at 23, and neither does the region and engagement read.
- The first level, 14.04, then takes the band from 23 down in one pass: 8.96 mm against a DPP of 6.0. The planner thinks those cells are already cut.

**(c) The D50 addendum guard does not see it either.** `rest_ceiling_keeps_doc` (`path.rs:387`) reads only the cells inside the centre containment (`:393`), and after the clear. The band is outside that containment (it is under the disc edge only) and already cleared.

**(d) Wanaka op 7.** It is the same class at a different place: its first passes meet stock that stands 14–16 mm above the draped floor near the outline (D61). That is the §16 band and outer-strip stock, which this planner does not count. Wanaka's op 1 is `Inside`, so its raw line is the material line, and the band beyond it is the frame. So the wanaka part of G-RESTDOC is the stock just INSIDE the outline that the op 7 disc meets from centres on its region edge (the G-EDGEBAND row: the cell row on the outline). Expect a different mechanism there; this is a reading only.

Still to confirm (asked from D10, read only):
- the line "Cleared cells outside boundary polygon cells=N" for op 5, and whether "DerivedRestRegions pre-boundary union did not collapse" is absent for op 5 (that is, it had a single pre-boundary);
- op 1's sim stock top over the band (expect about 23) and the highest stock inside the raw line (expect about 20.04).

## 3. Proposal: the smallest fix, in the ANCHOR

The rule: a rest op's first level sits at most one DPP below the highest stock its tool meets inside its containment.

- **The anchor, not the D50 guard.** D41 (no D50) shows the same 14.04, so the fault is in `level_anchor_z`, not in the ceiling. The D50 guard only decides whether the published top may LOWER the anchor, so it cannot raise an anchor that is already too low.
- **One definition, "the cells the tool meets".** These are the cells whose square touches a disc centred inside the op's centre containment (`Adaptive3dGeometry::centre_boundary`, empty = every cell). The test is the one the sliver-safe floor read uses: centre within R + cs/2 (`dexel_stock` `max_conservative_top_cell_in_disc`). No new number.
- **Read them before the boundary clear.** `level_anchor_z` takes this reach and reads the stock as the border clear left it, before the boundary clear (move the call above `:947`, or keep a copy of the column tops). The border clear still runs first, so phantom off-mesh cells stay out.
- **The guard uses the same reach.** `rest_ceiling_keeps_doc` reads the same reach set and the same pre-clear stock, so the anchor and the ceiling read one definition.

### Effect (to pre-register before code)

- Corne op 5 after the geodesic op 1: the anchor becomes 23 (the band), so the first level is 17.0 = 23 − DPP, as after a CP op 1 (D62). The level count may rise from 4 to 5. The 8.96 mm pass falls to at most one DPP, plus the drape. The time rises somewhat (one more level over the band).
- Shipped CP rest ops: unchanged wherever the previous op left nothing above the anchor within reach beyond the raw line. That is the normal CP case (D62: 17.0 both ways). They change only where that stock exists, which is this defect.
- Parity: the byte-parity fixtures and goldens are fresh-stock ops (`reads_prior_stock` false gives `stock_top_z`), so they do not change. Rest-op goldens, if any, change only in the case above. Run them to confirm.
- The rest op's region and engagement read still has the band cleared. So the band is cut at the right levels by the drape, but the planner's engagement there under-reads (the O5b side). Restoring that is the larger change: keep the reach band in the planner stock, as G-CLIPAIR does for the research arm. That decision is the lead's.

### Not proposed

Raising the published `rest_regions_top_z` to the band. The band is not in the published regions (G-EDGEBAND, parked), and the anchor fix covers this without it.
