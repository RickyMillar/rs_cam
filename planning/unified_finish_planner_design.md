# P2 — Unified finishing pass planner: design (DRAFT for review)

Status: drafted overnight 2026-07-07→08 (Fable, from a code survey; user review
pending before any implementation). Parent: `planning/unified_finishing_pass_plan.md`.

## Shape decision: one new op, orchestrating existing generators

`UnifiedFinish` becomes ONE new `OperationType` (X-macro row + registry entry +
one `generate_unified_finish` in execute.rs). Its generator is an ORCHESTRATOR:
it never re-implements a cutting strategy — it calls the existing region-scoped
free functions (`waterline_toolpath_with_cancel`, scallop, raster, pencil's
centerline machinery) once per region with a single-polygon `RegionSet`, then
routes and stitches the per-region toolpaths.

Why not multi-config orchestration (one ToolpathConfig per region/strategy):
regions are data-dependent (recomputed per mesh/tool), so configs would appear
and vanish on regenerate — fights the session model, the GUI list, and undo.
The planner's fan-out is an implementation detail of one op, exactly like
adaptive's regions are today. The session's 1-config-1-toolpath invariant holds.

## Pipeline

1. **Decompose** (new `finish_planner::decompose`):
   - `SlopeMap::from_z_grid` over a drop-cutter heightmap (reuse the sampling
     the drop_cutter/scallop path already does) → `classify_steep_shallow`
     mask at `steep_threshold_deg` (param, default 45°).
   - Mask → polygons via the EXISTING `region_polygons_from_mask` (move it
     from `rest_field.rs` to a shared module — it is the only mask→polygon
     extractor with dilation + containment; steep_shallow's per-point
     filtering is the anti-pattern this replaces). Sliver cap + pathology
     advice (d0d6d75) apply as-is.
   - Creases: `detect_rest_valleys` → `RestCenterline`s (points + half-width).
     Factor pencil's `resample_polyline` + `paths_from_sampled` offset logic
     into a callable that turns centerlines into cut paths without pencil's
     emission loop.
   - Output: `Vec<PlannedRegion { polygon, strategy, entry_candidates }>` +
     crease paths. Shallow = complement of steep within the machining
     boundary, minus crease corridors (creases are cut last, ridge-accurate).
2. **Assign**: steep → waterline rings; shallow → raster (default) or ring
   mode; creases → centerline paths. Strategy per region is a param-overridable
   default, not hardcoded.
3. **Generate per region**: each strategy free-function with
   `RegionSet::from_slice(&[region.polygon])`. PRE-REQ: `generate_drop_cutter`
   / the raster free function must accept `boundary_regions` (it is the ONE
   3D-finish generator not wired — survey confirmed; small task, mirrors the
   other seven).
4. **Route globally** (new `finish_planner::route`): greedy nearest-neighbor
   over region entry/exit points, seeded at the previous op's end; edge cost =
   `min(retract_link_time, surface_link_time)` where the surface candidate is
   drop-cutter-sampled (promote pencil's `build_surface_link` to a shared
   module) and only offered INSIDE the machining boundary — never across
   excluded islands (the selective-scallop gouge rule). 2-opt polish only if
   the A/B says greedy leaves >5% on the table (P0 discipline: measure first).
   Rings within a steep region: inside-out or outside-in by which endpoint
   chains cheaper to the next region (P3's inside-out preference plugs in
   here later).
5. **Stitch**: concatenate per-region toolpaths in route order, emitting the
   chosen link between each (surface link as Linking feed moves; retract link
   as Retract/Linking rapids). Span offsets remapped exactly like the clip's
   provenance mapping. The stitched result flows through the NORMAL session
   post-passes — boundary clip (usually no-op: generation was pre-clipped),
   `optimize_entry_descents` (stock-aware descents come free), feed
   modulation, F-034 accounting.

## What must move/generalize first (P2.a, mechanical)

- `region_polygons_from_mask` → shared module (rest_field keeps a re-export).
- `build_surface_link` → shared module (pencil keeps using it).
- Pencil centerline→paths factoring (`resample_polyline` + width-capped
  offset passes) into `rest_field` or a new `crease_paths` helper.
- `boundary_regions` wiring for the raster generator.
- Each is independently testable and useful; land before the op skeleton.

## Acceptance (per parent plan)

- A/B via `tests/p1_headless_ab_wanaka.rs` pattern: replace 3D Finish 6 (+
  pencil) with one UnifiedFinish op on the same setup; compare F-034 totals.
  Target: beat the P1-optimized stack (8920 s baseline), not the P0 one.
- 0 rapid collisions; sim deviation vs model unchanged; scallop-height spot
  checks on steep/shallow boundaries (the overlap_distance question —
  steep_shallow's dilation params carry over as planner params).
- steep_shallow op stays untouched (deprecation decision is the user's, later).

## Open questions for review

1. Steep strategy: waterline rings per region vs scallop-continuous confined
   to the region — scallop's rings follow the boundary shape (better for
   organic islands); waterline is strictly Z-leveled (better for true walls).
   Proposal: default scallop-continuous for steep islands whose aspect is
   blobby, waterline for wall-like (needs a cheap shape metric — or just a
   user param to start).
2. Crease corridors: subtract pencil half-width corridors from shallow
   regions, or let the shallow pass overlap and rely on the crease pass to
   clean? Proposal: overlap (simpler, no new geometry ops); measure scallop
   crests at corridor edges in the A/B.
3. Where does the planner get its heightmap: reuse the drop-cutter grid the
   raster strategy needs anyway (one sampling, shared), sampled at the
   FINEST stepover among assigned strategies.
