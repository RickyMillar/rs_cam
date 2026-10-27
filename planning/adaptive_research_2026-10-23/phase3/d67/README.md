# D67: G-GEOWATERLEAK (B2 312 Geodesic against 135 CP), read only

Code read: geodesic-integration HEAD `7503fa3f1` (I1 has uncommitted edits in `adaptive3d/clearing.rs`, so nothing was built or run in that tree). Line numbers are at `7503fa3f1`. Data asked from D10 (below).

## 1. Where the waterline cleanup gets its contour

- `adaptive3d/clearing.rs:1715` `waterline_cleanup`. The contour is `waterline_contours_with_cancel(mesh, index, cutter, z_level, sampling, cancel)` (`:1763`): the mesh contours at the level, from the mesh alone. The function takes no boundary (the test's mechanism 2, "boundary-unaware").
- Filters: the steep-only filter (slope > 30° on a third of the samples), and, for Geodesic Adaptive only, the skip of a contour that lies half or more inside a piece the arm left (`:1797`, `ctx.left_pieces`). This filter only REMOVES contours, so it cannot make more endpoints than CP.
- Each contour goes out through `push_segment_with_stamp` with `StampDrape { centre_clip: ctx.centre_clip, .. }` (`:1736-1750`). So in a shipped generation the planner's centre clip (G-BOUNDARYPHANTOM, the containment polygons) trims the waterline before it is pushed and stamped. In this fixture `centre_boundary` is empty (`base_params`, test line 142), so no centre clip runs.

## 2. What B2 counts (corrected after D10's probe, §7)

- B2 (`tests/adaptive3d_boundary_clear_parity.rs`, `measure`, about line 455) counts every non-rapid move of the PRE-CLIP planner output whose endpoint lies outside the containment rectangle.
- **On this fixture every B2 endpoint is a waterline cleanup `ClearingCut` (`Linear`), on both strategies:** CP 135 (133 sloped, 2 flat), Geodesic 312 (305 sloped, 7 flat). There is no `EntryPlunge`, `Linking`, `Retract` or arc move in it.
- My first reading here (that the extra endpoints were Geodesic's feed-rate routed travels, held links and plunges) was WRONG. The extras are waterline cuts.
- Why Geodesic Adaptive has more waterline endpoints: the lead's summary says "more waterline contours". The code allows only fewer: the Geodesic branch of `waterline_cleanup` (`:1797`) can only drop contours, and the contours come from the mesh alone (`:1763`). The code points instead to the same contours emitted as more, shorter moves. `emission_chord_tolerance` (`adaptive3d/mod.rs:402`) caps Geodesic's simplify tolerance at `slice_load_margin`, which is finer than CP's tolerance on this fixture, and B2 counts endpoints, not length. This is a reading, not measured. Either way the extras are waterline cuts. The check that decides between the two: count the waterline contours per strategy, and sum the XY length of the B2 moves (equal length means "same contours, more moves").

## 3. Can anything cut outside the boundary in a shipped job?

On the shipped path there are two clips, and neither runs in B2:

1. The planner's centre clip (`ctx.centre_clip`, from `Adaptive3dGeometry::centre_boundary`, which the session sets from the containment): every segment, the waterline included, is clipped before it is pushed and stamped.
2. The session's post-generation clip (`ProjectSession::apply_boundary_clip`, through `geometry/boundary.rs:279` `clip_toolpath_to_boundary_set_with_provenance`): every non-rapid move outside the clip polygon becomes a retract, a rapid and a re-entry descent at the plunge rate (G-BOUNDARYPLUNGE).

So a feed move outside the containment does not reach the program, on either strategy. The test itself simulates the clipped path. Its zone table (B3, "outside-containment sim_higher", counted, not gated) shows the cells outside are left standing, not cut. D10 is asked to confirm with the count of cells outside cut below the stock top (expect 0).

**The one exception, on both strategies:** a containment whose inset collapses (the tool is larger than the region, or the inset eats the region). Then `effective` is empty, the planner gets no centre clip, the session clip passes the path through, and only the report-only finding `boundary_clip_dropped` remains (`clip_annotated_to_boundary_set`, `boundary.rs:178-206`). In that case the boundary-unaware waterline and every other move outside ship. This is not specific to Geodesic, and it is the real exposure behind the "frame or clamp zone" question.

## 4. Verdict (pending D10's numbers)

- G-GEOWATERLEAK is a counting artefact of B2 on this fixture, not a cut outside the boundary:
  - B2 is pre-clip and counts non-cutting feed moves;
  - the shipped path clips twice.
  It is not a safety issue for Geodesic Adaptive as such.
- The cost it can carry on a shipped job is time: each waterline contour that crosses the containment is cut into pieces by the centre clip, and the session clip adds retract and re-entry pairs where a move leaves the clip polygon.

## 5. Smallest fix (proposal; no code)

1. **Make the bar measure what it names.** Count the B2 endpoints of the `ClearingCut` intent only (the waterline), or count after the shipped clips. Then CP and Geodesic read the same leak, and the per-strategy pin goes. Test-only.
2. **The real defect (D4 F-B, both strategies):** give `waterline_cleanup` the boundary. Drop a contour, or trim it, outside the centre containment before it is pushed. The centre clip already does this when `centre_boundary` is set, so the change is to pass the containment into the cut it makes, so that a waterline contour is not traced and re-entered across the boundary. Scope: `clearing.rs:1763-1850`, behind the existing `ctx.centre_clip`.
3. **The exposure (both strategies):** a collapsed boundary should refuse generation (as a missing boundary source does), not ship unclipped with a report-only finding. That is a product decision for the operator, and the only case where a cut outside a declared boundary can reach the machine.

## 6. D10's partial data (fix47, the unmodified test at a clean `7503fa3f1`)

| | CP | Geodesic |
|---|---|---|
| B2, pre-clip endpoints outside the containment | 135 | 312 |
| leak radius | 15.053–23.150 | 15.053–23.150 |
| max cut-endpoint x | 23.108 | 23.108 |
| inside containment⊖r / in the band | 607 / 428 | 3578 / 654 |
| OUTSIDE zone of the clipped sim (4005 cells): agree / planner higher / sim higher | 2 / 0 / 4003 | 69 / 0 / 3936 |

- The same leak radii and max x on both strategies fit item 2: the outside endpoints lie in the same place on both. Geodesic adds more moves there, and does not reach further.
- The 69 "agree" cells (|planner − sim| ≤ one cell) are not yet proof of a cut outside. Two readings fit:
  - (a) The clipped sim cut those cells down to near the planner's cleared floor: a cut outside.
  - (b) The planner did NOT clear them, and both stand near the stock top. Geodesic's planner keeps the stock within its reach beyond the raw boundary (the outer-band clip, G-CLIPAIR), where CP's plain boundary clear empties every cell outside.
  Reading (b) needs no cut outside. The probe decides it: for the 69 cells, print the planner top, the sim top and `STOCK_TOP_Z`.
- The probe (items 1, 2, 4 and the exact item 3) needs a test edit. D10 waits for the lead.

## 7. D10's probe (P15 "## D67"; `d67_waterleak_probe.rs`, `probe_output.txt` in this folder): the answer

At a clean `7503fa3f1`, release, test-support (the probe is removed and the tree is clean):

| | CP | Geodesic |
|---|---|---|
| B2 by section / intent / kind | 135: waterline cleanup, `ClearingCut`, `Linear` (133 sloped, 2 flat) | 312: waterline cleanup, `ClearingCut`, `Linear` (305 sloped, 7 flat) |
| B2 after `clip_toolpath_to_boundary` | 0 | 0 |
| B2 with `centre_boundary = effective` (as a session sets it) | 0 | 0 |
| clipped sim: cells outside the rectangle cut below the stock top | 42 (deepest 14.455) | 78 (deepest 14.399) |
| ... of which more than one cell (0.529 mm) outside | 0 | 0 (farthest cell centre 0.108 mm out) |
| unclipped path, for contrast | 3097 cells, up to 23.4 mm out | 3078 cells, up to 23.4 mm out |
| with `centre_boundary` set: edge-row cells | 43 | 20 (deepest 14.388, 0.108 mm out) |

### Correction of §2

My reading in §2 was WRONG: the extra endpoints are not Geodesic's feed-rate transits or plunges. All of B2 is waterline `ClearingCut` on both strategies. No `EntryPlunge`, `Linking` or `Retract` move is in it.

### Why Geodesic has more waterline endpoints (reading; the mechanism in code)

The waterline contours are the same mesh contours (the Geodesic branch only drops some). They go out through the emitter, which simplifies each cut with `Adaptive3dParams::emission_chord_tolerance` (`adaptive3d/mod.rs:402`). For Geodesic Adaptive that tolerance is capped at `slice_load_margin` (c·D − min(s, D); C26). On this fixture (stepover 1, tolerance 0.5) the cap is under the 0.5 mm that CP uses, so the same contour arc goes out as more, shorter `Linear` moves. B2 counts endpoints, not length, so it reads about 2.3 times as many. The same leak radius and max x on both strategies (§6) fit this. To confirm: the summed XY length of the B2 moves should be about equal on both strategies (not measured).

### Verdict

- **Not a safety issue, and not a cut outside the boundary.** B2 is a pre-clip count of the boundary-unaware waterline on a fixture whose planner has no centre clip. Both shipped clips bring it to 0.
- The clipped path cuts only the grid row that straddles the boundary (cell centres at most 0.108 mm out, from a cutter on the ⊖r ring): quantisation, on both strategies.
- The 2.3× is an artefact of counting endpoints under Geodesic's finer chord tolerance.

### Fix

- **No product fix is needed for G-GEOWATERLEAK.** The DEFECTS row can close as "counting artefact; no cut outside".
- **Test (smallest):** make B2 strategy-neutral. Either count the summed XY length of the waterline moves outside (equal on both strategies by the reading above), or measure with `centre_boundary` set as a session sets it (0 on both; then B2 pins the shipped behaviour, and the D4 F-B arm stays the ignored contract). Either removes the per-strategy pin.
- **Still open, both strategies, unchanged by this finding:**
  - D4 F-B: the waterline is boundary-unaware, so the planner traces contours across the boundary and the centre clip then cuts them up (time, not safety);
  - the collapsed-boundary pass-through (§3), the one case where a cut outside a declared boundary can ship. That is an operator decision.

## 8. D67b: the 69 Geodesic "agree" cells (D10; `d67b_agree_probe.rs`, `probe_b_output.txt`)

Fixture case (`centre_boundary` empty), clipped sim. Every outside cell with |planner − sim| ≤ one cell (0.529 mm). Stock top 25.0, planner floor 0.0.

| cells | where | planner top | sim top | reading |
|---|---|---|---|---|
| Geodesic 43 | one column at x = 2.754 (2.754 mm out, within R), y −11.0…11.2 | 25.0 | 25.0 | (b): not pre-cleared (the outer-band clip keeps the reach), not cut |
| Geodesic 26 | the edge row, 0.108 mm out | 18.35–20.50 | 18.81–20.54 | the legal edge cut from a cutter on the ⊖r ring |
| CP 2 | the edge row, 0.108 mm out | 17.98 / 18.37 | 18.40 / 18.77 | the same |

No cell beyond the edge row is cut, on either strategy. Reading (b) of §6 holds for 43 cells, and the other 26 are the quantised edge row.
