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

## Decisions (user review, 2026-07-08)

1. **Three bands, two thresholds** (user: scallop is faster, waterline is
   better on true steeps; both matter): shallow → raster; mid-steep →
   scallop-continuous rings; very-steep → waterline. Primary dial =
   `steep_threshold_deg` (default 45°); `waterline_threshold_deg` (default
   65°) is an advanced dial. Bands with no cells simply don't exist.
2. **Overlap is a parameter** (like steep_shallow's `overlap_distance`),
   default derived from the strategy stepover, not a magic constant.
3. **One heightmap**, sampled once at the finest stepover among assigned
   strategies; SlopeMap, decomposition, and all strategies read it.
   **AMENDED 2026-07-08 (P2.b measurement)**: classification and generation
   need DIFFERENT surfaces. The drop-cutter heightmap is the ball-CENTER
   offset surface, which geometrically hides steepness at feature scales at
   or below the ball radius — measured on wanaka (6 mm relief, Ø6 ball):
   38.5% of true surface area is ≥45° but only 0.1% of the offset surface
   reads that steep (max 52° vs true 89°); classifying on it produced a
   single all-shallow region. The user called it ("wanaka definitely has
   steep regions"). Decision: **classify on the TRUE surface**
   (`finish_setup::build_classification_surface_with_cancel` — tiny
   bare-surface probe per rest_field's precedent, grid cell-compatible with
   the generation surface), **generate on the offset surface** (unchanged
   per-strategy sampling). Band assignment is thereby tool-independent, so
   one decomposition can serve a multi-tool cascade. Corollary: the
   steep_shallow op classifies on the offset surface and shares this blind
   spot — quantified but left as-is (the planner supersedes it). `decompose`
   also erodes coverage by one cell before classifying (stencil-safe: slope
   at covered/uncovered boundaries reads the heightmap's min_z clamp and
   fabricates cliffs).
4. **Ball-tip tools only** (ball nose + tapered ball) via registry
   `tool_constraints`, like scallop today. Kills the flat/V-bit contact-
   geometry axis entirely.
5. **One-new-dial rule**: the op's genuinely new dials are the two threshold
   angles + overlap. Everything else (scallop height, stepovers, crease
   detector knobs, feeds) is inherited from the existing per-strategy params.

## Crease corridors: merge by default, split by measured width

The ridge detector already measures `half_width_mm` per centerline. Rule:
a crease stays INSIDE its surrounding zone (raster/rings cut across it;
they physically can't reach the crease bottom — that residual is exactly
what the pencil pass cleans) when `half_width < K × tool_radius` (K ≈ 2,
tunable); wider than that it's a canyon, not a crease — it becomes its own
zone. This answers merge-vs-split with data we already compute, and avoids
over-splitting shallow zones at every hairline valley.

## Risk register (self-review, 2026-07-08)

- **R1 — boundary fragmentation (the steep_shallow ghost; TOP RISK).** Slope
  oscillation around a threshold shreds the mask into islands. The design
  MUST include a region-conditioning stage before polygon extraction:
  hysteresis between the two thresholds (enter steep above T, leave below
  T−10°), morphological close, and MIN-AREA ABSORPTION (regions smaller than
  ~a few tool diameters² merge into their surrounding band rather than
  existing independently). The sliver cap + pathology advice (d0d6d75) stay
  as the backstop. Acceptance: region count on wanaka should be O(10), not
  O(100).
- **R2 — seam quality at band boundaries.** Ring/raster direction changes at
  a boundary can leave visible crests. Overlap param + scallop-height spot
  checks at boundaries in the A/B; if seams persist, boundary-following
  cleanup ring as a later increment.
- **R3 — routing ceiling is modest on wanaka; don't oversell.** Post-P1 the
  finishing stack's addressable linking is ~1000–1500 s of 9543; the router
  might recover a third to a half of that (~5–8% project). The equally real
  P2 payoff is ONE op replacing three + region-local quality choices. If the
  A/B shows less than ~5%, the planner still ships on the UX/quality merits
  but P3 (morphed spiral) takes priority for time wins.
- **R4 — ordering/decomposition tuning space.** The router itself (greedy +
  2-opt over tens of regions with integrator costs) is a solved shape; the
  real unknowns are decomposition parameters (thresholds, hysteresis,
  min-area, overlap, corridor K). Plan: expose them on the config from day
  one and drive a SWEEP HARNESS (param_sweep pattern + the headless-A/B
  scoring: integrator wall-clock, collision count, deviation/coverage) over
  wanaka + 2–3 synthetic fixtures (dome, cliff, branching valley). That is
  the deterministic version of the user's "monte carlo the maxima" idea —
  sweeps over the dials that matter, scored by the same integrator metric
  as everything else.
- **R5 — stitching side-data.** Merging per-region `AnnotatedToolpath`s:
  spans offset-remap (clip-provenance pattern, solved), but rest_grid /
  rest_regions / debug traces come from the crease detector only — the
  unified op must carry the detector's grid so the heatmap overlay works.
  Budget a PR for this; it's fiddly, not hard.
- **R6 — cancellation + progress.** Multi-region generation must poll cancel
  between regions and report phase progress (existing CancelCheck + debug
  trace patterns; MCP timeout/cancel already landed).

## Build order (P2.b onward)

1. P2.b — decomposition + conditioning (R1) with tests on synthetic fixtures;
   no toolpath emission yet, just regions + a debug/heatmap view.
2. P2.c — per-band generation + naive concatenation (no router), A/B
   checkpoint #1 (must not regress the P1 stack).
3. P2.d — router (greedy + link costing + 2-opt toggle), A/B checkpoint #2.
4. P2.e — decomposition-parameter sweep harness; lock defaults from data.
