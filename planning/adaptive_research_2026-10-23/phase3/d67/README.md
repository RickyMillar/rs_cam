# D67: G-GEOWATERLEAK (B2 312 Geodesic against 135 CP), read only

Code read: geodesic-integration HEAD `7503fa3f1` (I1 has uncommitted edits in `adaptive3d/clearing.rs`, so nothing was built or run in that tree). Line numbers are at `7503fa3f1`. Data asked from D10 (below).

## 1. Where the waterline cleanup gets its contour

- `adaptive3d/clearing.rs:1715` `waterline_cleanup`. The contour is `waterline_contours_with_cancel(mesh, index, cutter, z_level, sampling, cancel)` (`:1763`): the mesh contours at the level, from the mesh alone. The function takes no boundary (the test's mechanism 2, "boundary-unaware").
- Filters: the steep-only filter (slope > 30° on a third of the samples), and, for Geodesic Adaptive only, the skip of a contour that lies half or more inside a piece the arm left (`:1797`, `ctx.left_pieces`). This filter only REMOVES contours, so it cannot make more endpoints than CP.
- Each contour goes out through `push_segment_with_stamp` with `StampDrape { centre_clip: ctx.centre_clip, .. }` (`:1736-1750`). So in a shipped generation the planner's centre clip (G-BOUNDARYPHANTOM, the containment polygons) trims the waterline before it is pushed and stamped. In this fixture `centre_boundary` is empty (`base_params`, test line 142), so no centre clip runs.

## 2. What B2 counts

- B2 (`tests/adaptive3d_boundary_clear_parity.rs`, `measure`, about line 455) counts every NON-RAPID move of the PRE-CLIP planner output whose endpoint lies outside the containment rectangle. That includes:
  - waterline cuts (`Linear`/arc);
  - entry plunges (`EntryPlunge` feeds);
  - links at feed.
- The two strategies differ in what they emit at feed beyond the waterline:
  - Geodesic Adaptive defaults to `KeepDownRoute::CutFeed` (`adaptive3d/mod.rs:328`): its routed keep-down travels go out at the cut feed, so B2 counts them.
  - Geodesic Adaptive's held links are also feed moves.
  - CP's default route is `Off`.
  - The waterline contours are the same mesh contours on both strategies, minus the left pieces on Geodesic.
- **Reading (to be confirmed by D10's split):** the 177 extra endpoints are not more waterline. They are Geodesic's feed-rate transits (routed keep-down travels, held links) and their entry plunges near the waterline contours outside the rectangle. The R4 report's attribution to "other contours" is not supported by the code: the Geodesic branch of `waterline_cleanup` can only drop contours.

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
