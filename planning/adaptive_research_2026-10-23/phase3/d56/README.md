# D56 (option b of D55): status WIP, a ruling is needed

Tree: worktree `adaptive-head`, base `f2724a5bd` (D55). Not committed.
WIP patch: `tree_vs_f2724a5bd_WIP.patch` (md5 in the report). The pre-registration is `PREREG.md`.

## What is built

- `ProjectSession::rest_source_reach` (`session/compute/generation.rs`) resolves the source op's machining reach for a `DerivedRestRegions` consumer: the raw polygon for `Inside`, grown by the source tool radius for `Center`. It does so only for a single-polygon source boundary on the same setup and model. In every other case it gives `None`.
- The reach goes through `ResolvedGenInputs`, `ExecutionContext::rest_source_reach` and `VolumetricInputs::source_reach` to `Adaptive3dGeometry::source_reach`. The planner (`adaptive3d/path.rs`, after the boundary clear) clears every planner-stock cell whose centre is outside it.
- The PREREG explains why: a clear by the consumer's own regions that keeps the sliver-safe margin keeps exactly the cells the rapid-floor read looks at.
- Neutral without a consumer: parity 5/5 plain and research, the op golden 1/1, D44, D48 and D50 green. 20 `Adaptive3dGeometry` literals gain `source_reach: Vec::new()`.

## What the sentry found (`a_rest_entry_ignores_the_frame_beyond_the_source_reach_d56`, RED)

Fixture: two channels and a wide pocket that run to the mesh edge. Op 0 is the research arm with a silhouette boundary, `Inside`. Op 1 is CP on remaining stock, `DerivedRestRegions` with `Center`, and `Plunge`.

- The reach resolves for op 1: one polygon of 6439.9 mm², the silhouette. Op 0 publishes top 7.0 and only the two channel regions (centre space, x −37.25 to 37.25).
- With the clear, op 1's entries at the channel ends still start their descent from the stock top, and the keep-down links run at Z 10.5. One contact read is 9.375 (the first feed ends at 9.875). The frame is cleared, so this stock is INSIDE op 0's reach.
- The stock is the research arm's §16 outer band. Op 0 does not cut between its centre containment and its reach: `ResearchClip::restrict_regions` makes no region there, "the edge contour pass cuts it". The band is not in `Produced::rest_regions` either. It is real stock beside the consumer's disc. The sliver-safe read is right to read it, and no correct clear may remove it.
- An experiment that also cleared the row of cells on the reach line did not change the floors.

## Consequence for wanaka op 7 (not measured)

At the edge sites the stock that holds the floor at the stock top may be this band, not the frame. A full-height sliver of the band at the disc edge would also explain why op 7 removes only 1.8 cm³ in 916 descents. If so:
- D56 does not reduce op 7's entry time.
- D55 is the right fix: it feeds straight past the sliver and pecks only where the column has stock.

To decide, one fix37 run on this tree (D55 + D56) against one on `f2724a5bd` (D55 alone) gives op 7's entry time and the rapid floors at the edge sites.

## Options for the lead

1. Measure first (one wanaka s2.0 run on each tree, about 30 s each plus the sim). Commit D56 only if op 7's entry time falls.
2. Drop D56. Keep D55, and treat the §16 band as the edge op's work.
3. Separate from D56: publish the §16 band in `rest_regions`. Today the edge op's regions omit the band it is meant to cut, and it reaches the band only with its disc edge.

## Lead ruling (D56)

Option (1) goes to D10 as D57: wanaka s2.0 on D55 alone, then on D55 plus this WIP patch. D56 stays WIP until then, with no commit request. Then the lead decides between (2), keeping D56, and (3). Below is option (3) as a design only.

## Option (3), design only (no code): publish the left band in `rest_regions`

### The problem

The edge op is the op that §16 sends the band to ("the outer strip and the stock-edge band go to a separate contour pass", `research_outer_band.rs:6-7`). But today its regions are only the research arm's left PIECES (D44: "no field" and "F-038 forecast"). So the edge op reaches the band only with the edge of its disc, from a centre in a piece region. Its entries there read the band as a sliver at full height. That is the D56 sentry's contact read of 9.375, and probably wanaka's edge sites.

### Where the band comes from (line numbers at `f2724a5bd` plus the WIP patch)

Two different things share the name.

**(a) The outer strip: inside the source's machining reach.**
- The 2D planner plans a region outline that is first inset by `SIMPLIFY_INSET` (0.3 mm) and simplified with `SIMPLIFY_TOLERANCE` (0.3 mm) (`clearing.rs:3176-3179`, `smoothed_region`). Then each piece is inset by the tool radius (`planner_outlines`).
- The comment there bounds the stock left at the region edge by about 0.6 mm. Along a region edge that a wall does not bound (a boundary, a level cut), that strip stands at the level's stock top.
- C31 grown outlines (`grown_planner_outlines`, clipped by `clip_to_centre_reach`, which keeps `PERIMETER_INSET_MARGIN_MM`, 0.25 mm, inside the clip) change its width per region.
- This strip is stock the source left INSIDE the region it was meant to clear. It is the edge op's work.

**(b) The reach band: from the raw boundary out to the raw boundary grown by R.**
- G-CLIPAIR (`path.rs:1004` area; `boundary_reach`, `research_outer_band.rs:148`) keeps the stock that a disc centred inside `geometry.boundary` reaches. `ResearchClip::restrict_regions` (`:178`) makes no region there.
- `geometry.boundary` is the RAW boundary polygon. With `Center` containment it is centre space, so the band is inside the source's machining reach, and it is the edge op's work.
- With `Inside` containment (wanaka) the raw polygon is the material line, so the band is OUTSIDE the machining reach: it is the holding frame. It must never be published.

### What to publish, and in which space

1. **A left-stock mask over the source's machining reach.** After the run, read the source's own final planner stock (`Adaptive3dSegmentsResult::final_material_stock`, `path.rs:706`). Mark each cell inside the source reach (the D56 `rest_source_reach` polygon, raw for `Inside` and grown by R for `Center`) whose stock stands above THE level floor (`intended_floor::level_floor_z`) of the lowest level by more than the op tolerance (`Adaptive3dGeometry::tolerance`, an existing value).
   - This is one rule for (a), for the in-reach part of (b) and for the pieces themselves. It is generic, so any strategy can publish it (§13.8).
2. **Convert it to tool-centre space, as D48 requires.** Extract the mask polygons (`geometry::region_mask::region_polygons_from_mask_reported`, `region_mask.rs:147`, as the rest field does). Grow them by the source tool radius, because a centre within R of left stock can cut it. Then clip them to the source's centre containment (`Adaptive3dGeometry::centre_boundary`, the D48 clip) and union them with the pieces.
   - The D48 guarantee holds unchanged: a `Center` consumer with a tool no larger than the source's (D53) stays inside the source's boundary.
3. **Z top for D50.** `rest_regions_top_z` becomes the highest stock top over the mask cells. The planner stock gives it directly, so the level-list rule (`slab_top_z`) is no longer needed for these cells. It still bounds only the START of the consumer's levels, with the one-Depth/Pass guard of the D50 addendum.
   - Expect the top to rise to the stock top wherever the outer strip at the first level is left. On wanaka that undoes most of D50's level cut (op 7 back to 4 levels) unless the per-region Z (D48 proposal a/b) comes first. The lead must weigh this.

### Expected effect (not measured)

**Wanaka op 7** (source `Inside`, offset 0):
- The regions gain a ring along the outline: the outer strip, about 0.6 mm of stock inside the outline, grown by R and clipped to the centre containment. That puts the centres on the centre-containment line.
- Op 7 then cuts the strip as a contour along the edge. Today it plunges beside the strip from piece regions.
- The band inside 0–3 mm (P15 D47: 2012 mm² left after op 7 at s3.5, 1495 at s2.0) should fall toward CP's 0–3 mm value. CP also leaves this row, because its `Inside` boundary keeps the tool off it.
- The entries on the ring still read the strip. D55 feeds through it and pecks only where the column holds stock, so the 537 s of entry time should fall as the plunges along the edge become one contour.
- Cut time rises by the ring: about 800 mm of perimeter at each level where the strip stands.
- The frame is never published, because the reach for `Inside` is the raw polygon.

**Corne op 5** (source `Center`, offset +6):
- The reach is the silhouette grown by 6 mm and then by R. The in-reach band (b) is the outer stock-edge ring 6 to 9 mm outside the part, and (a) runs along it.
- Op 5 gains that ring as a region. P15 D49 has 193 mm² outside 0–3 mm after op 5. Expect the outer rings (3–9 mm) to clear, at the cost of a ring pass.
- Op 5's time rises by the ring length. Its peck sites are already one descent (median 1), so no entry time is saved.

### Risks and checks before any code

- The mask must not take the source's `stock_to_leave` skin as left stock. The floor is `level_floor_z` with the source's own leave, so the skin is not above it.
- With a thick leave the consumer's own (smaller) leave makes it cut the skin, which is today's behaviour.
- Cell quantisation: a mask cell centred inside the reach can hold material only partly inside the reach. The growth by R and the D48 clip keep the consumer's centre inside the source containment, so its disc never passes the reach line. A mask cell beyond the line is not in the mask.
- Byte parity: the mask runs only on demand (`run_volumetric` drops `rest_regions` with no consumer). With no consumer the output is byte-identical.
- Sentries to pre-register:
  - the edge op's regions contain the outer strip along a boundary (a fixture like the D56 sentry's);
  - no published region lies outside the source's centre containment (D48);
  - the frame is never published for an `Inside` source;
  - the published top bounds every published cell.
