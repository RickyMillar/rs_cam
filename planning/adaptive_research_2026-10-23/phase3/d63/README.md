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

## 4. Build attempt (lead approval), stopped: the anchor fix alone does not hold the rule

WIP patch: `tree_vs_f2724a5bd_WIP.patch` (md5 in the report), base `f2724a5bd`. The pre-registration is `PREREG.md`. Not committed.

### What is built

- One shared reach test, `tool_reach_cells` (`adaptive3d/path.rs`): a cell is in reach when its centre lies inside the centre containment grown by R + cs/2 (the sliver-safe test). An empty containment means every cell.
- One snapshot, `PriorStockTops::read`: the column tops of the reach cells, taken after the border clear and before the boundary clear, on prior stock only.
- `level_anchor_z` and `rest_ceiling_keeps_doc` read only that snapshot.
- Unit tests: `the_anchor_reads_the_stock_the_tool_meets_d63` (a band in reach counts, stock beyond reach does not); the D50 and G-LADDERANCHOR unit tests are moved onto the snapshot and pass.
- Parity: `adaptive3d_emission_byte_parity` 5/5 plain and research, `every_op_keeps_its_toolpath_g_opgolden` 1/1. No fixture moved.

### What the sentry found

The sentry `a_rest_op_starts_one_depth_under_the_band_it_meets_d63` uses a planted case:
- CP source, `Center` on the silhouette, 1 mm leave;
- CP rest op on remaining stock, `Center` on the silhouette + R/2, no leave;
- the stock stands 8 mm past the model, so a band at the stock top (10) lies from the silhouette + R to the silhouette + 1.5R. That is beyond the rest op's raw line and inside its reach.

| tree | rest op's first emitted pass | max simulated axial engagement |
|---|---|---|
| without the fix (snapshot after the boundary clear) | Z 1.0 | 9.0 mm (DPP 3.0) |
| with the fix | Z 1.0 | 9.0 mm |

A probe in the fixed tree: the snapshot holds 19362 reach cells out to |x| 35, with max top 10. So the anchor is 10, and the plan holds the levels 7, 4 and 1.

But the levels 7 and 4 emit no move. The boundary clear has removed the band from the planner stock, and at those levels no cell INSIDE the raw line holds stock. A level region is a set of centre cells with stock under them, so these levels have no region. The band is then cut only at Z 1, by the edge of the disc, 9 mm deep.

So "the first level is at most one DPP below the band" now holds for the plan, but not for the cut. The approved fix does not meet the lead's rule: "no level cuts deeper than one DPP plus the drape".

### Consequence for Corne op 5 (expected, not measured)

The anchor fix helps only where stock inside the raw line puts regions at the upper levels. There the disc edge takes the band one level at a time. Along the raw edge where the inside is already low, the band is still taken at a low level, more than one DPP deep.

### What the rule needs (for the lead to decide)

1. **The region side (INTEGRATION_PLAN step 3, "G-CLIPAIR for every strategy").** The band stays in the rest op's planner stock (the reach clear of G-CLIPAIR in place of the plain boundary clear), and a level region also holds the centre cells inside the containment whose DISC meets stock at that level, not only the cells with stock under the centre. This is the change that holds the rule. It moves shipped output for bounded ops on prior stock, and needs its own pre-registration.
2. **An edge pass per level.** A contour along the containment line at every level where the reach band holds stock above the level (the §16 "separate contour pass" inside the op). Smaller in scope, but a new pass.
3. **Refuse or warn.** The export or generation refuses a rest op whose simulated axial engagement exceeds one DPP (the axial rule only warns today, D49). This does not fix the cut, but it stops it from shipping silently.

The anchor change itself is safe (no parity move) and is a precondition of 1 and 2. It can be committed alone with a weaker sentry (the plan's first level), or held until 1 or 2 is chosen. The sentry above stays red until then.

## 5. The anchor fix as committed (lead ruling) and its sentries

Patch: `tree_vs_f2724a5bd.patch` (md5 in the report), base `f2724a5bd`. It holds `adaptive3d/path.rs`, `adaptive3d/tests.rs`, `adaptive3d/CLAUDE.md` and the new sentry. It replaces the WIP patch.

Sentry file: `tests/a_rest_op_starts_one_depth_under_the_band_it_meets_d63.rs` (not research-gated; about 4 s per run). It holds three tests on one planted CP + CP pair:

| test | asserts | old order (snapshot after the clear) | this patch |
|---|---|---|---|
| `a_rest_op_plans_its_first_level_one_depth_under_the_band_d63` (the weaker sentry) | planned first level (`z_top` of the "Compute Z levels" span) is at most one DPP under the band top | RED: 1.000 | green: 7.000 |
| `g_restdoc_the_band_is_still_cut_deeper_than_one_depth_d63` (the known red) | today's defect: max simulated axial engagement above DPP + one sim cell, and the first EMITTED pass below band − DPP | passes (9.0 mm, pass at 1.0) | passes (9.0 mm, pass at 1.0) |
| `the_d62_planted_case_plans_one_depth_under_the_band_d63` | the D62 case (raw line inside the band) plans its first level at band − DPP | passes (7.0) | passes (7.0); emitted 7.0, max axial 3.0 |

**About the form of the known red.** The lead asked for `#[ignore]` with a reason. `every_ignore_states_its_kind_f152` refuses that: a reason must start with `instrument:`, `evidence:`, `heavy:`, `external:` or `serial:`, and `tests/CLAUDE.md` says "a known red is an inverted assertion that runs and fails when the fix lands". So the strong sentry is an inverted assertion that runs. It names G-RESTDOC and "the level region holds no centre cell over the reach band", and it fails, with a message that says to turn it into the rule, when the region side lands.

**Gates (all through the lane, in the foreground):**
- fmt; clippy, full features with and without research: ok.
- `adaptive3d_emission_byte_parity` 5/5 plain and research; `every_op_keeps_its_toolpath_g_opgolden` 1/1. No re-bless.
- The 13 adaptive3d folder sentries; f152; f140; D44 x2, D48, D50 (arm env); D53; D55; `generic_rest_routing_pr7`; `rest_region_tip_radius_dilation_f1`: all ok.
- `rs_cam_core --lib` 2705 plain and 2756 research; `rs_cam_cli` 92.

## 6. Decision page for the operator: G-RESTDOC, what holds "one Depth/Pass per level" on a rest op

**The defect.** A 3D Rough on remaining stock with a boundary can meet stock that a previous op left in the strip one tool radius beyond its boundary line. With `Center` containment its tool reaches past that line. Its planner clears that strip before it plans, so no level has work there, and the strip is cut at a low level by the edge of the tool: up to 9 mm against a DPP of 3 in the planted CP + CP case, 8.96 against 6.0 on Corne op 5 (D61).

It reaches shipped CP whenever the rest op's boundary line lies inside the previous op's band: a rest op with a smaller offset than its source, or the same offset with a smaller tool on the source. The anchor fix above gets the PLANNED first level right in every case. The cut needs one of the options below.

### Option 1: the region side ("G-CLIPAIR for every strategy", INTEGRATION_PLAN step 3)

- **Scope.** Two changes:
  - On prior stock, the planner keeps the stock within the tool's reach beyond the raw boundary (the reach clear of G-CLIPAIR in place of the plain boundary clear), for every strategy, not only the research arm.
  - A level region holds the centre cells INSIDE the containment whose disc meets stock above the level, not only the cells with stock under the centre.
  Files: `adaptive3d/path.rs` (boundary clear), `adaptive3d/clearing.rs` (region build, `build_material_bool_grid` and its callers), `research_outer_band.rs` (`restrict_regions` generalised).
- **Shipped output moved.** Every bounded 3D Rough on prior stock whose source left stock in its reach band: new cuts at the upper levels along the boundary, and different entries and links there. Fresh-stock ops: the region rule could also change where a wall within one radius of the boundary has stock above the level. That must be measured; it is NOT expected to be zero. Byte parity and the op golden would likely move.
- **Risk.** Medium to high. The region model is the core of every adaptive3d strategy. The O5b failure mode (planned cuts deleted by the clip, unstamped cells, deep bites later) is guarded by G-BOUNDARYPHANTOM's centre clip, but a new region rule must be re-proven against it. The engagement read in the band also changes (it is no longer read as cleared), which changes feeds and load verdicts there.
- **Recommendation.** This is the complete fix and the one the rule needs. Do it as its own programme step with pre-registration and a measured parity report. Do not do it as a patch on a defect.

### Option 2: an edge pass per level

- **Scope.** At each level, after the regions, one contour along the centre-containment line, wherever the reach band (from the D63 snapshot, `PriorStockTops`) holds stock above that level. It is the §16 "separate contour pass" inside the op. New code in the level loop (`clearing.rs`), using the existing waterline-cleanup emit path.
- **Shipped output moved.** Only bounded ops on prior stock with stock in the band: an added contour per affected level. No change to the region model, so fresh-stock ops and ops with nothing in the band are byte-identical (expected; to be confirmed by parity).
- **Risk.** Low to medium. A new pass needs its own entry and link handling and its own load check: a full-height band one radius wide is a slot-like cut on the disc edge. It does not fix a band that a region edge meets inside the raw line (the G-EDGEBAND row). It adds cut time along every affected boundary.
- **Recommendation.** The best ratio of safety to scope. It holds "one DPP per level" for the band case without touching the region model. Recommended if Option 1 is not scheduled soon.

### Option 3: refuse (or warn) on the simulated axial depth

- **Scope.** Generation or export refuses a rest op (or any op) whose simulated axial engagement exceeds one Depth/Pass by more than a stated margin. The axial depth rule exists today but only warns ("Exceeded without refusing the export", D49; the rigidity rule of thumb). Code: `tool_load` / the export policy (`gcode::enforce_load_policy`), the same place as the other refusals.
- **Shipped output moved.** No toolpath changes. Exports that breach are refused (with an override, as the other load refusals have).
- **The G-AXIALDOC point.** Shipped CP already has local over-DPP spots. On Corne, CP op 1 has 2.9 % of its volume over DPP, max 11.25 mm, and CP's 3 mm copy op has 13.1 %, max 12.0 mm (D62). A refusal rule keyed on "any sample over DPP" would refuse those shipped jobs too. So the rule must be one of:
  - (a) a volume share: refuse only above a stated share of the op's removed volume over DPP;
  - (b) a depth cap: refuse only above a stated multiple of DPP;
  - (c) refuse only for ops on remaining stock (the G-RESTDOC class) and warn for the rest.
  Every threshold in (a) or (b) is a new number, and by the project rule it must be derived or taken from a source, not typed in. (c) needs no number but leaves G-AXIALDOC as a warning.
- **Risk.** Low for the code. Product risk: it refuses existing CP jobs unless the threshold is chosen with them in view, and it makes no cut safer by itself.
- **Recommendation.** Use it as a guard beside Option 1 or 2, not instead of them. Choose (c) first: refuse for rest ops, which is exactly G-RESTDOC, and keep the warning for G-AXIALDOC until that class has its own fix.

### Summary

| option | holds the rule | moves shipped output | risk | recommendation |
|---|---|---|---|---|
| 1 region side | yes | bounded rest ops; maybe fresh ops near walls | medium–high | the full fix, as its own programme step |
| 2 edge pass | yes, for the band case | bounded rest ops with stock in the band only | low–medium | recommended next step |
| 3 refuse | no (it stops shipping) | no toolpath change; refusals | low (code) | as a guard, form (c), beside 1 or 2 |
