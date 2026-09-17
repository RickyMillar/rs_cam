# T2 — Island machinery: decomposition, routing, filtering

**Track T2, 2026-08-23. Read-only investigation.** Every claim carries a
`file:line`. Line numbers are against the working tree at investigation time
(branch `master`, HEAD `bc5fba45`). Uncommitted pencil-related edits by a
concurrent lane were ignored.

**Headline:** the island machinery the multi-tool plan needs is **already
built and shipped**, in two independent layers that already meet. A tier map
does not need a new decomposition engine; it needs a *label source*. The
single most important structural finding is in §2.4: only **one** of the five
boundary sources reaches the pre-generation per-cell mask that `unified_finish`
decomposes against. Pick the wrong one and the tier confinement degrades from
"plan on the tier" to "plan on everything, then clip".

---

## 1. unified_finish's band/region decomposition and island routing

### 1.1 The pipeline, end to end

Two modules. `finish_planner.rs` decides *what the regions are*;
`unified_finish.rs` decides *what is cut in each one and in what order*. The
split is explicit: the planner "emits **no toolpaths**; it is purely the
decomposition stage that feeds the P2.c per-band generation orchestrator"
(`crates/rs_cam_core/src/finish_planner.rs:7-10`).

`decompose` (`crates/rs_cam_core/src/finish_planner.rs:352-548`) runs seven
numbered steps against a `SlopeMap` + a `covered: &[bool]` mask + a crease set:

| Step | What | Lines |
|---|---|---|
| 0 | Stencil-safe coverage erosion (1 cell, unconditional) | `finish_planner.rs:380-407` |
| 1 | Hysteresis classification → `steep`, `very` bool masks | `finish_planner.rs:409-431` |
| 2 | Morphological close on both masks | `finish_planner.rs:433-441` |
| 3 | **Three-way label grid** `Vec<Option<FinishBand>>` | `finish_planner.rs:443-459` |
| 4 | Min-area absorption | `finish_planner.rs:461-463` |
| 5 | Crease corridor claims (carve `labels` → `None`) | `finish_planner.rs:465-478` |
| 6 | Per-band polygon extraction (marching squares) | `finish_planner.rs:480-511` |
| 7 | Stats / provenance | `finish_planner.rs:513-541` |

Output is `PlannedRegions { regions: Vec<PlannedRegion>, creases, stats }`
(`finish_planner.rs:309-317`), where `PlannedRegion { band: FinishBand, polygon:
Polygon2 }` (`finish_planner.rs:233-238`).

### 1.2 The COLUMNS instrument

**COLUMNS is not part of the decomposition pipeline.** It is a *quality
measurement* harness that scores `SimulationResult::column_deviations`, used
for A/B comparison of classifier and strategy arms — it lives in tests, not in
`src/`:

- `crates/rs_cam_core/tests/strategy_comparison_h4.rs:72` — "**COLUMNS quality**
  (`SimulationResult::column_deviations`)"
- `crates/rs_cam_core/tests/classification_columns_ab_m3.rs:9` — the M3 A/B is
  "**scored on `SimulationResult::column_deviations`** (COLUMNS)"
- Scoring is "indexed by (row, col), never XY"
  (`crates/rs_cam_core/tests/strategy_comparison_h4.rs:582`)
- The hook that lets both classifier arms run through one production pipeline
  is `UnifiedFinishParams::classification_sampler`
  (`crates/rs_cam_core/src/unified_finish.rs:179-185`), "a field rather than a
  global so the COLUMNS A/B can run both classifiers through the identical
  production pipeline in one process".

**Implication for T2:** COLUMNS is the instrument that would *adjudicate* a
tier map (does tiering hurt surface finish at the seams?), not a component to
modify. It needs a committed terrain fixture
(`crates/rs_cam_core/tests/classification_columns_ab_m3.rs:144`).

### 1.3 Region nodes and `route_greedy`

Regions become **nodes** in a routing graph:

- Node type: `struct RegionPath { region_index, band, tp, anns, head_strip,
  entry, tail_strip, exit }` (`crates/rs_cam_core/src/unified_finish.rs:2334-2350`).
  `head_strip`/`tail_strip` are the removable rapid+plunge preamble and
  trailing retracts, so a surface link can replace them.
- `route_greedy(paths, mesh, index, cutter, params, machining_boundary, lk,
  cancel)` (`unified_finish.rs:2420-2486`): greedy nearest-by-**integrated
  link time**, seeded at the steepest band present (`unified_finish.rs:2430-2438`).
  2-opt is deliberately absent (`unified_finish.rs:2417-2418`).
- Edge cost: `choose_link` (`unified_finish.rs:2497-…`) costs
  `min(retract_link_time, surface_link_time)`; the surface candidate is
  gouge-checked **and required to stay inside the machining boundary** — "the
  selective-scallop gouge rule: never feed across excluded islands"
  (`unified_finish.rs:2492-2494`).
- Fallback with no `LinkKinematics`: steep-first band-major concat, native
  links only, **never a distance-costed guess** (`unified_finish.rs:2155-2169`);
  the P0 probe measured distance/feed misjudging segmented paths by up to 10×
  (`unified_finish.rs:2157-2158`).
- Stitching preserves node `move_range` **tiling**, including the incoming
  link, because `tsp::split_into_segments` only splits on `Rapid` and a link
  left outside every node range gets relocated by the TSP reorder — measured on
  wanaka ×2, five dropped region nodes carrying **87%** of the op's cutting
  length, leaving `narrate_toolpath` reporting `regions 0`
  (`unified_finish.rs:2222-2239`).

`route_greedy` is `O(n²)` in region count (`unified_finish.rs:2448-2475`: a
scan over all unvisited candidates per step), and each `choose_link` runs a
gouge-checked surface-link build. **This is the routing-side cost driver if
island counts explode** (see §3.4).

### 1.4 Can the region set be driven by an EXTERNAL mask? — YES, three seams

**Seam A (already wired, no code): `machining_boundary` ANDed into `covered`.**

```
pub fn unified_finish_toolpath_with_cancel(
    …, machining_boundary: Option<&RegionSet<'_>>, …)
```
`crates/rs_cam_core/src/unified_finish.rs:1297-1311`.

Step 2 (`unified_finish.rs:1337-1363`) walks every classification cell and ANDs
`boundary.contains(&P2::new(x, y))` into `covered` **before `decompose` runs**
(`decompose` is called at `unified_finish.rs:1662`). An external polygon set
therefore already confines the decomposition itself — the bands, their
conditioning and their extraction all happen inside the supplied islands. This
is the *cheapest possible* tier-mask injection and it costs zero new code in
the core. The op adapter already passes `ctx.boundary_regions` into this slot
(`crates/rs_cam_core/src/compute/execute.rs:2244`).

**Seam B (already wired, no code): `territory_clip`'s mask-AND.** A *raster*
keep-mask is ANDed into `covered` before `decompose`, at
`unified_finish.rs:1577-1643`. The mask is built from the rest field
(`unified_finish.rs:1592-1596`), dilated by EDT on the rest grid
(`unified_finish.rs:1606-1609`), then **nearest-resampled onto the
classification grid** (`unified_finish.rs:1610-1635`). This is a
working, shipped example of *exactly* the operation a tier-map raster needs —
including the resample between two grids of different pitch. It is gated on
`CreaseReference::MachinedStock` + a `territory_stock`
(`unified_finish.rs:1581-1589`), so it is not directly reusable for a
slope/curvature tier map, but the ~50 lines are the template.

**Seam C (needs code): the label grid itself.** Steps 1–3 of `decompose`
(`finish_planner.rs:409-459`) are the *only* thing that decides band membership.
Everything downstream — absorption, crease claims, extraction — is generic over
`labels: Vec<Option<FinishBand>>`. Substituting an external tier raster for
steps 1–3, keeping 0 and 4–7, is a surgical change. See §5 for the trade-off
against Seam A.

### 1.5 Invariants regions must satisfy

Established by reading the consumers:

1. **Closed, correctly wound `Polygon2` with holes grouped.**
   `region_polygons_from_mask_clamped` calls `detect_containment` then
   `ensure_winding` on every result (`crates/rs_cam_core/src/region_mask.rs:158-161`).
   Exterior CCW / holes CW is asserted in its tests (`region_mask.rs:235-245`).
2. **Disjoint by construction.** `RegionSet` "does not itself enforce or rely on
   non-overlap beyond 'membership is inside ANY region'"
   (`crates/rs_cam_core/src/region_set.rs:15-16`). Overlap is *tolerated* for
   containment, but see the area-summation warning below.
3. **Must not touch the grid boundary.** Marching squares cannot close a loop
   that touches the edge; production masks always carry a non-contact margin
   ring (`region_mask.rs:366-370` — a test fixture silently lost ten regions to
   this). `decompose` step 0 guarantees it by construction
   (`finish_planner.rs:393-396`).
4. **Minimum area one cell.** Loops with `< 3` points or enclosed area under
   `cell²` are dropped before grouping (`region_mask.rs:151-156`).
5. **Sorted largest-exterior-area-first, deterministically tie-broken**
   (`region_mask.rs:166-180`). Downstream `route_greedy` seeding relies on
   "first planned region of that band — deterministic"
   (`unified_finish.rs:2414-2415`).
6. **Areas are XY-projected and NOT summable across bands when
   `overlap_mm > 0`** — `PlannedRegion::projected_xy_area_mm2`
   (`finish_planner.rs:240-255`) says so explicitly, and
   `DecomposeStats::provenance` records both facts (`finish_planner.rs:297-306`).
7. **A region with no covered cells inside its own polygon is skipped**, with a
   warning, on the VerySteep arm (`unified_finish.rs:1757-1764`).

### 1.6 How a region becomes cut moves

Per region, one strategy call with a **single-polygon** `RegionSet`
(`unified_finish.rs:1748-1750`):

```
for (region_index, region) in planned.regions.iter().enumerate() {
    let region_set = RegionSet::new(vec![region.polygon.clone()]);
```

then dispatch on `region.band` (`unified_finish.rs:1755`):

- `VerySteep` → `waterline_toolpath_with_cancel(…, Some(&region_set), cancel)`
  (`unified_finish.rs:1818-1828`), with a **per-region Z range** so each region
  only ladders the levels its own patch spans (`unified_finish.rs:1816-1817`).
- `MidSteep` → `scallop_toolpath_structured_annotated_with_cancel(…,
  Some(&region_set), cancel)` (`unified_finish.rs:1869-1877`). Scallop's own
  slope filter is deliberately a **no-op** here — "the REGION is the
  confinement now (the band's polygon already IS the slope-selection)"
  (`unified_finish.rs:1839-1845`).
- `Shallow` → `raster_toolpath_from_grid(grid, …, Some(&region_set))`
  (`unified_finish.rs:1983-1990`), sharing one mesh-global drop-cutter grid
  across all shallow regions (`unified_finish.rs:1727-1732`).

**The `Option<&RegionSet>` confinement parameter is the universal seam across
the mesh-finish family** — same shape in `scallop.rs:1893`, `1927`, `1991`,
`2027`, and `radial_finish.rs:86`. Filtering is **point containment on cutting
targets**: `boundary_regions.is_none_or(|regions| regions.contains(&P2::new(p.x,
p.y)))` (`crates/rs_cam_core/src/scallop.rs:2260`;
`crates/rs_cam_core/src/radial_finish.rs:115`).

Intra-region relinking uses **the region's own polygon** as its boundary,
because "an intra-region link that leaves it cuts territory the decomposition
… deliberately excluded" (`unified_finish.rs:2015-2018`).

---

## 2. Boundary machinery

### 2.1 `ToolContainment` — three variants, radius-aware

`crates/rs_cam_core/src/boundary.rs:13-22`:

| Variant | Semantics | Line |
|---|---|---|
| `Center` | tool **center** stays inside; boundary unchanged | `boundary.rs:16-17`, `:26` |
| `Inside` | entire tool stays inside; **shrinks by `tool_radius`** | `boundary.rs:18-19`, `:27` |
| `Outside` | tool edge may extend outside; **expands by `tool_radius`** | `boundary.rs:20-21`, `:28` |

Implementation at `boundary.rs:66-79`: `Inside` → `offset_polygon_reported(b,
+tool_radius)` ("positive = inward for CCW exterior", `boundary.rs:75`),
`Outside` → `-tool_radius` (`boundary.rs:77`), `Center` → unchanged
(`boundary.rs:74`).

`effective_boundary` "may return multiple polygons if the offset splits the
shape, or empty if it collapses" (`boundary.rs:30`). An empty result "is not
self-explanatory and its two causes are not equally safe" — production sites
use `effective_boundary_reported` (`boundary.rs:32-34`). This is the
`boundary_clip_dropped` finding CLAUDE.md warns about: **`Inside` is not an
unconditional guarantee.** Applied per-region in the multi-region path, where it
"may split one region into several (or collapse it to none)"
(`session/compute.rs:2156-2157`, flattened `:2159-2177`) — so region **count is
not preserved** across containment.

Config-side mirror is `BoundaryContainment { Center, Inside, Outside }`
(`crates/rs_cam_core/src/compute/config.rs:1426-1431`), mapped 1:1 at
`session/compute.rs:2150-2154`.

### 2.2 `BoundaryConfig` — the whole dial surface

`crates/rs_cam_core/src/compute/config.rs:1482-1490`:

```
pub struct BoundaryConfig {
    pub enabled: bool,
    pub source: BoundarySource,
    pub containment: BoundaryContainment,
    /// Additional offset in mm (positive = expand, negative = shrink).
    /// Applied after source resolution, before tool-radius containment.
    pub offset: f64,
}
```

Default is `enabled: false`, `source: Stock` (`config.rs:1492-1496`). It "can
live on `StockConfig` (global default) or on individual `ToolpathEntry`
(per-toolpath override)" (`config.rs:1480-1481`), with a `boundary_inherit`
flag on the toolpath (`crates/rs_cam_core/src/session/mod.rs:675-676`).

**Sign-convention trap.** `BoundaryConfig::offset` is stated as *positive =
expand* (`config.rs:1487`). It reaches `RegionSet::processed(keep_outs,
offset)` (`session/compute.rs:1268`, `:2146-2147`), which internally applies
`offset_polygon(&poly, -offset)` (`region_set.rs:101`) because `offset_polygon`
itself takes positive = **inward** (`region_set.rs:168-171`). The two negations
cancel; the config-level statement is the correct one. Do not "fix" either
sign in isolation.

### 2.3 `BoundarySource` — five variants

`crates/rs_cam_core/src/compute/config.rs:1436-1457`:

| Variant | Payload | Line |
|---|---|---|
| `Stock` (default) | stock bounding rectangle | `config.rs:1437-1439` |
| `ModelSilhouette` | 3D model projected along Z | `config.rs:1440-1441` |
| `Geometry` | `{ polygon_indices: Vec<usize> }` — DXF/SVG closed chains | `config.rs:1442-1444` |
| `FaceSelection` | selected STEP/CAD faces projected to XY | `config.rs:1445-1446` |
| `DerivedRestRegions` | `{ source_toolpath_id: ToolpathId }` | `config.rs:1447-1456` |

`ALL_SIMPLE` lists only `Stock` and `ModelSilhouette` as combo-box-safe; the
other three each need a picker (`config.rs:1460-1465`).

`DerivedRestRegions` carries a deliberate design property: the id is the
**stable** `ToolpathConfig.id`, not an index, and "the regions are re-resolved
from that result at generation time, never copied in here, so they always
reflect the source's latest generation" (`config.rs:1449-1453`).

### 2.4 ⚠ The structural finding: only `DerivedRestRegions` reaches the pre-clip

There are **two** places a boundary is applied, and they are fed differently.

`resolve_generation_inputs` (`crates/rs_cam_core/src/session/compute.rs:1259-1316`):

- The `DerivedRestRegions` arm resolves the source's polygons, runs
  `RegionSet::from_slice(&regions).processed(&keep_out_footprints,
  boundary_config.offset)`, and stores the result in **`pre_boundary_regions`**
  (`session/compute.rs:1261-1276`).
- **Every other source** falls to the `else` branch, which computes only
  `pre_boundary` — a *single* containment polygon for adaptive3d's internal
  pre-clip (`session/compute.rs:1299-1313`). `pre_boundary_regions` stays
  `None` (initialised `None` at `session/compute.rs:1259`).

`pre_boundary_regions` is what becomes `ExecutionContext::boundary_regions`
(`session/compute.rs:1514` → `compute/execute.rs:2892`, `:2912`), which is what
`generate_unified_finish` hands to the core as `machining_boundary`
(`compute/execute.rs:2244`) — i.e. **the Seam-A per-cell mask of §1.4**.

Everything else gets only the **post-generation** clip,
`apply_boundary_clip_multi` (`session/compute.rs:2127-2157`).

**Consequence for the tier plan.** Confining a `unified_finish` op to tier
islands via `BoundarySource::Geometry` (imported SVG/DXF tier polygons) would
**not** confine the decomposition. The op would classify, condition, decompose
and generate over the *whole* surface and then have the result clipped — paying
full generation cost, and worse, sizing its bands, its absorption and its
`overlap_mm` extraction against a surface it is not going to cut. Only
`DerivedRestRegions` currently gets tier confinement *before* `decompose`.

### 2.5 Can an island set become a machining boundary for a second unified op TODAY?

**Yes — config-only, and it is a shipped, MCP-reachable cascade.** The chain:

1. **Producer.** Any toolpath with `rest_analysis.enabled` gets
   `attach_generic_rest_analysis` run after generation
   (`compute/execute.rs:2994`, defined `:3023-3091`), which sets
   `generated.rest_grid` and `generated.rest_regions`
   (`compute/execute.rs:3089-3090`). This is explicitly "op-agnostic … available
   to every operation family, not just pencil's `RestDepth` detector arm"
   (`session/mod.rs:677-682`). `unified_finish` also emits its own
   (`unified_finish.rs:1548-1549` → `compute/execute.rs:2338`).
2. **Wiring.** Setting a consumer's boundary to `DerivedRestRegions` **auto-enables
   the producer**: `auto_enable_rest_analysis_for_source`
   (`session/mutation.rs:508-540`) flips `rest_analysis.enabled` on the source
   and invalidates its cache. It no-ops for a `rest_depth` pencil source, which
   attaches the artifacts itself (`session/mutation.rs:518-522`).
3. **Consumer.** `BoundarySource::DerivedRestRegions { source_toolpath_id }` →
   `pre_boundary_regions` → per-cell mask before `decompose` (§2.4).
4. **MCP.** Both ends are on the wire: `set_boundary_config` with
   `source: "derived_rest_regions"` + `source_toolpath_id`
   (`crates/rs_cam_viz/src/mcp_server.rs:1080`;
   `crates/rs_cam_mcp/src/server.rs:589-605`) and `set_rest_analysis_config`
   (`crates/rs_cam_viz/src/mcp_server.rs:1108`). **`Geometry` and
   `FaceSelection` are NOT on the MCP wire** — the param doc lists only
   `"stock"`, `"model_silhouette"`, `"derived_rest_regions"`
   (`crates/rs_cam_mcp/src/server.rs:595`).

Resolution is a single fail-hard chokepoint:
`ProjectSession::resolve_derived_rest_region_polys`
(`session/compute.rs:1794-1843`) **refuses** — it does not silently degrade —
when the source id doesn't exist (`:1804-1808`), is self-referential
(`:1811-1817`), has no generated result (`:1828-1831`), or produced no regions
(`:1834-1841`). Three call sites that must agree share it: the
`generate_toolpath` precondition (`session/compute.rs:1407-1413`), the
pre-boundary resolution (`:1265`), and the enforcement clip (`:1591`). The
producer side names the contract too: `RestFieldResult::region_polygons` is
documented as "the derived-boundary source for selective finishing
(`BoundarySource::DerivedRestRegions`)" (`rest_field.rs:374-381`).

GUI parity: the worker mirrors the core pre-clip
(`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:105-120`) and calls the
*same* core `apply_boundary_clip_multi` rather than re-deriving it, explicitly
to avoid drift (`execute/mod.rs:689-728`, note at `:693-705`). The editor has a
"Rest Regions" source option gated on `has_rest_candidates` plus a
source-toolpath picker (`crates/rs_cam_viz/src/ui/properties/mod.rs:4059-4134`,
`:4145-4154`).

**Three caveats before treating this as the finished feature:**

- **The tiering criterion is REST DEPTH, not slope or curvature.** Islands are
  "where the reference floats more than `min_valley_depth` above the true
  surface" (`config.rs:1529-1531`). For "fine tool goes where the coarse tool
  couldn't reach" that is arguably the *physically correct* criterion — but it
  is not the operator's stated "mountains vs flats".

- ⚠ **The island dilation is sized off the ENVELOPE (shank) radius — which on
  this project's tapered balls is a measured island-welding failure.** The mask
  is dilated by `pencil.radius() + params.region_margin_mm`
  (`rest_field.rs:830-836`), and the field doc is explicit that "Dilation
  radius is the fine (pencil) cutter's own **ENVELOPE** radius plus this
  margin" (`rest_field.rs:137-145`). For a pencil op that is deliberate — the
  envelope is what sweeps, and a boundary-clipped op must reach the true region
  edge. For a **Ø1-tip / Ø6-shank tapered ball** it means `3.0 + 0.5 = 3.5 mm`
  of dilation on islands whose features are tip-scale. That is the *same
  number* whose effect `territory_clip` documents having measured and
  deliberately avoided: "NOT `cutter.radius()`: for tapered tools that is the
  SHAFT radius … and a 3.5 mm dilation welds a dendritic keep-mask into full
  coverage (measured, run 5: Op B all-over again at 58.5 k s)"
  (`unified_finish.rs:1599-1603`). **Expect the shipped cascade to weld a
  mountain tier map into one giant region on exactly the tools this project is
  about.** There is no dial that reaches it — `region_margin_mm` is additive on
  top, not a replacement (`rest_field.rs:835`). This is the same
  envelope-vs-cusp class as the `for_tool` defect in §3.2 and the `radius()`
  items on the radius programme ledger.
- **Tool-role inversion.** `attach_generic_rest_analysis` uses **this
  toolpath's own** `tool_def` as the FINE cutter (`compute/execute.rs:3027`,
  `:3060`, `:3088`) and `reference_tool_id` (or stock, or a nominal probe) as
  the reference. So the op that must *carry* the analysis is the one holding
  the **fine** tool — while the boundary consumer reads a **different**
  toolpath's regions. Getting "R0.5 taper vs R2.0 ball" therefore needs an
  analysis-carrier toolpath holding the fine tool, not the coarse pass.
  Additionally, `resolve_rest_reference` gives **stock priority over
  `reference_tool_id`** (`compute/execute.rs:3110-3126`): if a machined-stock
  snapshot overlaps the mesh in XY, the named reference tool is silently
  ignored.

### 2.6 Representation: polygons, everywhere past extraction

Boundaries are `Polygon2` **polygons** at every application point —
`RegionSet` wraps `Cow<'a, [Polygon2]>` (`region_set.rs:29-31`). Rasters exist
only *upstream* of extraction:

- raster → polygon: `region_polygons_from_mask_clamped`
  (`region_mask.rs:88-207`), marching squares at `region_mask.rs:149`.
- The only raster-domain confinement in the whole finishing path is
  `territory_clip`'s mask-AND (`unified_finish.rs:1592-1635`), and the
  `machining_boundary` → `covered` test, which is **polygon → raster**
  (point-in-polygon per cell, `unified_finish.rs:1354-1361`).

The raster stage is internal-only to two producers: `model_silhouette`
rasterises then marching-squares back to `Vec<Polygon2>`
(`boundary.rs:385-419`, extraction at `:407-418`), and `detect_rest_valleys`
keeps its `rest_grid` separate from `region_polygons` — the grid is for the
heatmap, the polygons are what feed boundaries (`rest_field.rs:373` vs `:381`).

### 2.7 Which ops actually consume a region set

`ExecutionContext::boundary_regions` (`compute/execute.rs:653-662`) is read by
**nine** generators; the exceptions matter:

| Op | Adapter | Reads `boundary_regions` | Filter mode |
|---|---|---|---|
| Scallop | `execute.rs:2020` | yes `:2061` | **both** — region used as the literal ring-cascade boundary replacing the mesh-bbox rect (`scallop.rs:2183-2220`) **and** per-point containment (`scallop.rs:2259-2261`) |
| UnifiedFinish | `execute.rs:2098` | yes `:2244` | per-cell coverage mask (`unified_finish.rs:1354-1361`) + per-region wrap (`:1750`) + link containment (`:2613`) |
| SteepShallow | `execute.rs:2345` | yes `:2371` | point containment (`steep_shallow.rs:253`, `:465`) |
| RampFinish | `execute.rs:2389` | yes `:2416` | point containment (`ramp_finish.rs:774`) |
| SpiralFinish | `execute.rs:2442` | yes `:2464` | point containment (`spiral_finish.rs:204`) |
| RadialFinish | `execute.rs:2483` | yes `:2503` | point containment (`radial_finish.rs:115`) |
| HorizontalFinish | `execute.rs:2516` | yes `:2536` | point containment (`horizontal_finish.rs:244`) |
| DropCutter | `execute.rs:2549` | yes `:2638`, `:2647` | point containment (`toolpath.rs:658`, `:826`) |
| Waterline | `execute.rs:2659` | yes `:2686` | point containment (`waterline.rs:225`) |
| ProjectCurve | `execute.rs:1760` | yes `:1884` | only as `surface_link::LinkOptions.boundary` (`surface_link.rs:159`) — link decision, not a clip |
| **Pencil** | `execute.rs:1922` | **no** | not region-gated at generation; pencil is normally the *producer* of regions |
| **Adaptive3d** | `execute.rs:1593` | **no** — uses single-polygon `ctx.boundary` (`:1691`) | needs one containment polygon, hence `single_union` |

Scallop's dual use is worth noting for a tier design: it is the only op where
the region polygon is not merely a filter but the **seed geometry** of the
offset cascade — which is why the D-16.1 run-off (§4.2) mattered there and
nowhere else.

**Every** op still gets the post-generation enforcement clip when
`boundary_config.enabled`; `boundary_regions` is a generation-time pre-clip
optimisation, never the sole enforcement.

### 2.8 Keep-outs

`RegionSet::processed(keep_outs, offset)` subtracts keep-outs **per region**
before offsetting, and drops a region that collapses entirely rather than
erroring (`region_set.rs:92-109`; subtraction `:98-100` strictly before offset
`:101-104`). Backed by `boundary::subtract_keepouts`
(`boundary.rs:142-150`), which reverses each keep-out to CW winding and appends
it as a **hole**, so `contains_point` excludes it naturally.

Sources are per-setup: `Fixture::footprint()` and `KeepOutZone::footprint()`
(`session/mod.rs:429-459`), combined at `session/compute.rs:1070-1083` and
transformed to setup-local coords on non-identity setups (`:1119-1121`).

**Composition order is invariant across every boundary source**, single-polygon
and multi-region alike: subtract keep-outs → apply user `offset` → apply
tool-radius containment (`session/compute.rs:1895-1925` vs `:2146-2177`).
`apply_boundary_clip_multi` explicitly mirrors "what `resolve_containment_polygon`
does to its single polygon" (`session/compute.rs:2143-2145`).

---

## 3. The 1000s-of-islands problem

### 3.1 Morphological cleanup that EXISTS

Everything the plan calls for is already implemented. All of it is
whole-grid-EDT based (`O(cells)`), not per-ring loops.

| Operation | Function | Location |
|---|---|---|
| **Hysteresis** (dual-threshold flood fill) | `hysteresis_mask` | `finish_planner.rs:593-637` |
| **Dilate** | `distance_transform_2d` + threshold | `region_mask.rs:103-111` |
| **Close** (dilate∘erode, double-EDT) | `morphological_close` | `finish_planner.rs:740-751` |
| **Erode** | `NOT(dilate(NOT m))` trick | `finish_planner.rs:747-750` |
| **Coverage erosion** (1-cell stencil safety) | inline step 0 | `finish_planner.rs:388-407` |
| **8-connected labelling** | `label_components` | `finish_planner.rs:680-709` |
| **Per-band labelling** | `banded_components` | `finish_planner.rs:719-733` |
| **Min-area absorption** (majority-vote merge) | `absorb_small_regions` | `finish_planner.rs:761-837` |
| **Min-cell-count drop** | `MIN_REGION_CELLS = 4` | `rest_field.rs:87`, applied `:813-824` |
| **Merge-by-dilation** | `region_polygons_from_mask(dilate_mm)` | `region_mask.rs:49-57` |
| **Coverage clamp** on dilation | `…_clamped(clamp)` | `region_mask.rs:88-140` |
| **Containment grouping** (island-in-hole) | `detect_containment` | `region_mask.rs:158` |
| **Hard region cap** | `MAX_REST_REGIONS = 64` | `region_mask.rs:27`, applied `:182-204` |

**Nothing morphological needs to be written.** `absorb_small_regions` is
notably sophisticated: up to 4 passes because a reassignment can shrink a
neighbour below threshold too (`finish_planner.rs:756-757`), deterministic
smallest-first ordering (`finish_planner.rs:775-779`), majority vote among
outside neighbours (`finish_planner.rs:788-820`), and — importantly — **a
genuinely isolated island with no banded neighbour is left alone rather than
force-merged** (`finish_planner.rs:807-812`).

### 3.2 The dials and how they are sized

`FinishPlannerParams` (`finish_planner.rs:110-178`), derived by
`for_tool(cusp_radius)` (`finish_planner.rs:211-222`):

| Dial | Default | Source |
|---|---|---|
| `steep_threshold_deg` | 45.0 | `finish_planner.rs:213` |
| `waterline_threshold_deg` | **75.0**, not 65 | `finish_planner.rs:214`, ledgered FP-65 at `:116-125` |
| `hysteresis_deg` | 10.0 | `finish_planner.rs:215` |
| `overlap_mm` | 0.0 in `for_tool`; **2.0** in `UnifiedFinishConfig::default` | `finish_planner.rs:216`; `compute/operation_configs.rs:1112` |
| `close_radius_mm` | `cusp_radius × 0.5` | `finish_planner.rs:219` |
| `min_region_area_mm2` | `(2 × cusp_radius)² × 4` | `finish_planner.rs:220` |
| `crease_own_region_half_width_mm` | `2.0 × cusp_radius` | `finish_planner.rs:217`, `CREASE_OWN_REGION_K` at `:185` |
| `pencil_claim_floor` | `cusp_radius × 0.25` | `finish_planner.rs:218` |

**Load-bearing warning for a multi-tool build.** These dials MUST be sized off
`cusp_radius()` (tip), never `radius()` (shank). On a Ø1 tip / Ø6 shank taper
the wrong scale made `min_region_area_mm2` **144 mm² instead of 4** and
`close_radius_mm` **1.5 mm instead of 0.25**, which "closed and absorbed every
steep ribbon on terrain that is 25% steeper than 55°"
(`finish_planner.rs:204-210`; the adapter's matching note at
`compute/execute.rs:2129-2137`). Since the whole point of this project is
**fine tapered balls on mountains**, this is precisely the failure mode the
tier design will walk into if any new dial is sized off the envelope.
`hysteresis` is separately called load-bearing: "0 → the raw masks storm to
O(100) islands" (`finish_planner.rs:203-204`).

### 3.3 Island-count telemetry that already exists

`DecomposeStats` (`finish_planner.rs:280-307`) proves the conditioning did
work: `raw_steep_islands` / `raw_very_steep_islands` are counted **before**
conditioning (`finish_planner.rs:430-431`), `absorbed_regions` and final
`region_count` after. Logged as one `tracing::info!` line carrying the
measurement provenance alongside the counts (`finish_planner.rs:532-541`).

Acceptance target on record: "region count on a real mesh should be **O(10),
not O(100)**" (`finish_planner.rs:38-40`), sentried by
`finish_planner_wanaka_decompose.rs` (`finish_planner.rs:40`).

Operator-facing pathology classifier: `RestRegionPathology::TooManyIslands`
fires at `> MAX_REST_REGIONS / 2` (33+), *before* the cap has to truncate
anything (`rest_field.rs:1066-1073`, `:1094-1096`, `:1127-1131`); the opposite
pathology `SingleGiantRegion` fires at ≥50% of the **covered footprint** —
denominator explicitly not a bbox (`rest_field.rs:1074-1088`, `:1113-1122`).

### 3.4 ⚠ The three ceilings a tier map will hit

1. **`MAX_REST_REGIONS = 64` — a hard truncation.** `region_polygons_from_mask`
   keeps only the largest 64 by area and `warn!`s with the dropped area
   fraction (`region_mask.rs:182-204`). **This applies to every band extraction
   in `decompose`** (`finish_planner.rs:498-505`) — i.e. up to 64 *per band*,
   silently discarding the rest. The rationale is anti-sliver-storm: "a genuine
   rest-region job is a handful to a few dozen islands; the 2026-07-06 incident
   that motivated this cap produced hundreds" (`region_mask.rs:17-27`).
   Deliberately **no area floor beyond the cap**, because "a genuine long thin
   stripe of real rest material is not distinguishable from a sliver by area
   alone" (`region_mask.rs:45-48`). A terrain tier map with 1000s of mountain
   islands would be truncated to the 64 biggest, with only a log line.
2. **The 256 warn in the orchestrator** (`unified_finish.rs:1675-1681`) —
   soft, no hard fail: "the A/B gates catch pathology"
   (`unified_finish.rs:1673-1674`).
3. **`route_greedy` is O(n²)** with a gouge-checked surface-link build per
   candidate (`unified_finish.rs:2448-2475`), and per-region generation
   rebuilds scallop/waterline surfaces per call — accepted only because
   "O(1) regions per band on wanaka today", explicitly "flagged as a P2.e
   datapoint if conditioned region counts ever grow"
   (`unified_finish.rs:1725-1732`). That condition is exactly what a tier map
   would trigger.

### 3.5 Prior art: `steep_shallow`'s cruder overlap

The predecessor implementation dilates with a per-ring loop,
`dilate_grid(grid, rows, cols, radius_cells)`
(`crates/rs_cam_core/src/steep_shallow.rs:85`), applied symmetrically to both
the steep and shallow grids by `overlap_distance` (default **2.0**,
`steep_shallow.rs:39`, `:67`; applied `:742-751`). `finish_planner` explicitly
supersedes it: `morphological_close` avoids "the O(cells × radius) per-ring
dilation loop `steep_shallow::dilate_grid` uses"
(`finish_planner.rs:30-32`). A naive threshold classification of this kind is
named as the planner's top risk — "the steep_shallow ghost", which "shreds into
dozens to hundreds of tiny islands" (`finish_planner.rs:12-19`).

### 3.6 The `arp1_zones.pgm` lineage — a false lead, plus a reusable tool

**The zone rasters are not machining-zone tooling.** They are written by a
live, non-`#[ignore]`d test,
`render_the_plate_before_any_verdict_is_read()` at
`crates/rs_cam_core/tests/reference_plate_contract.rs:1119-1199`, which samples
a 900×900 grid over the **synthetic ARP-1 reference plate** and emits binary
`P5` PGMs (`reference_plate_contract.rs:1172`) for height, slope and zones
(`:1165-1168`).

"Zones" there means **which analytic feature-tile owns each pixel** — the
`Zone` enum is `{ Datum, Dome, Bowl, Saddle, ConeLadder, UGrooveComb,
VGrooveComb, StepTerrace, MicroRipple }`
(`crates/rs_cam_core/tests/common/reference_plate.rs:168-178`), a 4×4 lattice of
24 mm tiles on a 96×96 mm plate (`reference_plate.rs:768`, `:853`), resolved by
`zone_at(x, y)` (`reference_plate.rs:901-903`). It is surface *attribution* for
instrument verification, deliberately built to avoid "cluster-and-hope" and
dilated band maps (`reference_plate.rs:41-45`). ARP-1 "is **not**
representative and must never be quoted as if it were"
(`planning/review_2026-08-04/ORCHESTRATION_LOG.md:161`;
`reference_plate.rs:27-32`).

**What is salvageable:** the ~50-line dependency-free PGM writer
(`reference_plate_contract.rs:1119-1175`) is a ready-made pattern for
rasterising any `(x,y) -> label` function to an inspectable image. Given the
repo's own rule — *never gate on an aggregate without rendering the surface*
(`reference_plate_contract.rs:1107-1113`; MEMORY.md, v3 process-proof) — a
tier-map build should dump its label grid this way from day one.
`finish_planner::planned_regions_to_svg` (`finish_planner.rs:978`) already does
the polygon-side equivalent, with per-band colours (`finish_planner.rs:958-964`).

---

## 4. Tier-boundary blending

### 4.1 Overlap machinery that exists — four independent dials

1. **`overlap_mm`** — the intended inter-band dial. Band polygons are dilated at
   extraction "so neighbouring bands overlap" (`finish_planner.rs:130-133`),
   applied at `finish_planner.rs:498-505`. Config `UnifiedFinishConfig::overlap_mm`
   (`compute/operation_configs.rs:964`), default **2.0**
   (`operation_configs.rs:1112`), copied to the planner at
   `compute/execute.rs:2142`. **Note the default mismatch**: `for_tool` sets 0.0
   (`finish_planner.rs:216`) while the shipped config sets 2.0 — direct library
   callers and production disagree by default.
2. **`BoundaryConfig::offset`** — positive expands the boundary, applied after
   source resolution and before tool-radius containment (`config.rs:1487-1489`).
   Live-settable over MCP (`crates/rs_cam_mcp/src/server.rs:597-598`).
3. **`ToolContainment::Outside`** — expands by `tool_radius`, letting the cutter
   *center* reach the island edge (`boundary.rs:20-21`, `:28`). An automatic
   one-radius overlap, independent of (2).
4. **`region_margin_mm`** — extra clearance when dilating a rest mask into
   region polygons, **on top of the envelope radius**: `pencil.radius() +
   params.region_margin_mm` (`rest_field.rs:830-836`), default 0.5
   (`rest_field.rs:156`, `config.rs:1568`), "so a boundary-clipped op can reach
   the region edge. See P2.2 `BoundarySource::DerivedRestRegions`"
   (`rest_field.rs:826-829`). ⚠ The `pencil.radius()` term is the **shank** on a
   tapered tool and is not dial-reachable — see the §2.5 caveat. On the R0.5
   taper this dial's *effective* value is 3.5 mm, not 0.5.

So an inter-tier overlap band is available **today, config-only**, by stacking
(2)+(3)+(4) on the fine op's boundary. Dial (1) is intra-op only. Note (4) is
not independently controllable downward: it has a hard 3.0 mm floor on the
shipped taper.

### 4.2 The D-16.1 lesson — clamp the dilation, and clamp it to COVERAGE

Dilating a region polygon outward has a documented failure mode. At the shipped
`overlap_mm = 2.0` a mid-steep band polygon "ran ~2 mm past the last covered
classification cell, and that polygon is the scallop ring-cascade SEED, so the
first several rings sat entirely outside the model footprint"
(`region_mask.rs:70-83`). The fix was `region_polygons_from_mask_clamped`'s
`clamp` parameter, applied **after** the EDT so the distance transform still
sees the true mask (`region_mask.rs:113-117`).

Two rules stated in the source, both of which a tier design must inherit:

- **Clamp to COVERAGE, never to the band's own mask** — clamping to the band
  "would reduce every region to its own undilated footprint and reintroduce
  that seam" (`region_mask.rs:62-72`).
- A mismatched clamp length is a **caller bug**, warned and ignored rather
  than silently truncating against the wrong grid (`region_mask.rs:126-138`).

Reproduction fixture: `crates/rs_cam_core/tests/band_run_off_reproduction_d16_1.rs`.

### 4.3 `stock_to_leave` and the seam-step arithmetic

`UnifiedFinishParams::stock_to_leave` (`unified_finish.rs:142-164`) is a **+Z
vertical shift on the drop-cutter contact point**, applied identically in all
three bands: mid-steep scallop lifts each ring vertex, shallow raster lifts
each grid point (`unified_finish.rs:1973-1977`), very-steep waterline lifts
each contour point (`unified_finish.rs:1808-1812`). "Surface links and the
crease/claims pencil ride at the same offset, so **no band seam steps**"
(`unified_finish.rs:147-148`).

The approximation is stated in-source: what remains measured **normal** to a
wall at slope θ is `stock_to_leave · cos θ` — **0.71× at 45°, 0.26× at 75°**
(`unified_finish.rs:158-160`; matching the shallow-band note at `:1960-1965`).

**The history is the design lesson for tiering.** Before D-16.2/F3
(2026-08-06) only the scallop band honoured the dial. The two broken bands were
fixed **together, deliberately**: "fixing shallow alone would have introduced a
`stock_to_leave`-sized step at the shallow↔waterline seam that did not exist
before" (`unified_finish.rs:150-156`).

**Direct consequence for multi-tool tiers.** Two tools finishing adjacent
territory at the *same* nominal `stock_to_leave` leave the *same* vertical
residue and no step — the offset is a property of the dial, not the tool. But
the residue *normal to the surface* differs with local slope, so a tier
boundary deliberately placed **along a slope change** (which is exactly what
"fine tool on mountains, coarse on flats" means) puts a `stock_to_leave ·
(cos θ_flat − cos θ_steep)` normal-residue discontinuity right at the seam.
At `stock_to_leave = 0.1` across a 20°→70° tier boundary that is
`0.1 × (0.94 − 0.34) ≈ 0.06 mm`. **Mitigation: run the finish tier pass at
`stock_to_leave = 0`**, or hold it equal across tiers and accept the residue as
uniform-in-Z. Whether it should become a surface-normal offset is on record as
a separate repo-wide question, "deliberately NOT coupled to this field"
(`unified_finish.rs:161-163`).

### 4.4 The seam that actually prints as a step

Note what `stock_to_leave` does *not* cover: a step at a tier seam comes from
the two tools leaving different **cusp** heights, not from the offset dial. The
mid-steep band holds a scallop cusp while the shallow band holds a raster
stepover; the sweep note on `steep_threshold_deg` records that "scallop's held
cusp is coarser than raster's effective cusp in the 35–45° band, so
speed-hunting via this dial costs quality there" (`finish_planner.rs:196-198`).
A tier map swapping R0.5 for R2.0 changes the cusp by `stepover²/(8R)` — the
overlap band (§4.1) hides the *transition*, it does not equalise the two
finishes. Adjudicate with COLUMNS (§1.2), not by eye.

---

## 5. Seam verdict

### 5.1 Injection points, ranked

**Rank 1 — `machining_boundary` (Seam A). Zero core code. Use this.**
`unified_finish_toolpath_with_cancel(…, machining_boundary: Option<&RegionSet>,
…)` (`unified_finish.rs:1305`) ANDs the island set into `covered` at
`unified_finish.rs:1345-1363`, *before* `decompose` (`:1662`). The tier islands
confine classification, conditioning, extraction, generation and routing in one
move. Already fed from config via `ctx.boundary_regions`
(`compute/execute.rs:2244`).

**Rank 2 — a raster mask-AND on `covered` (Seam B shape).** Model on
`territory_clip` (`unified_finish.rs:1577-1643`), which already demonstrates
EDT dilation on a source grid plus nearest-resample onto the classification
grid. Correct choice if the tier map is natively a raster and you want to avoid
a polygonisation round-trip. Needs a new `ClaimsConfig`-sibling dial.

**Rank 3 — external label grid (Seam C).** Replace `decompose` steps 1–3
(`finish_planner.rs:409-459`) with an injected `Vec<Option<FinishBand>>`, keeping
steps 0 and 4–7. Only worth it if a tier must map to a *strategy* other than the
three `FinishBand` variants (`finish_planner.rs:97-104`) — and note
`FinishBand` is hard-wired 3-valued through `band_rank`/`band_from_rank`
(`finish_planner.rs:328-342`), `BAND_ORDER` (`:322-326`), the `votes: [usize; 3]`
array in `absorb_small_regions` (`:793`), the `RegionTableEntry` kind
(`unified_finish.rs:2258`), and the strategy `match` (`unified_finish.rs:1755`).
Widening it is a real refactor.

**Do NOT use `BoundarySource::Geometry`** for tier confinement despite its
apparent fit (SVG/DXF tier polygons). It never populates `pre_boundary_regions`
(`session/compute.rs:1259`, `:1299-1313`), so it degrades to a post-generation
clip only (§2.4), and it is not on the MCP wire
(`crates/rs_cam_mcp/src/server.rs:595`).

### 5.2 Representation: raster to produce, polygons to consume

Author and condition the tier map as a **raster** on the classification grid —
that is where every morphological primitive lives (§3.1) and where the
conditioning discipline that beats the steep_shallow ghost is proven
(`finish_planner.rs:12-40`). Convert **once**, at the end, via
`region_polygons_from_mask_clamped` (`region_mask.rs:88-207`), clamped to
coverage. Consume as `Polygon2` / `RegionSet` — that is the only representation
any downstream confinement accepts (§2.6).

### 5.3 Filtering: exists vs must be written

**Exists (all of §3.1):** hysteresis, dilate, erode, close, 8-connected
labelling, min-area majority-vote absorption, min-cell-count drop,
merge-by-dilation, coverage clamp, containment grouping, deterministic
area-sorted output, a hard 64-region cap, plus island-count telemetry and a
two-sided pathology classifier. **No morphological primitive needs writing.**

**Must be written (four items, all small):**

1. **The tier label source** — whatever maps `(slope, curvature, reach, local
   feature scale)` → tool index. This is the only genuinely new algorithm.
2. **A tip-scaled dilation for tier islands.** The derived-regions path dilates
   by the **envelope** radius (`rest_field.rs:830-836`, `:137-145`), which welds
   a tapered-ball tier map into one region (§2.5, third caveat). Either give the
   dilation a cusp-scaled arm, or bypass it by feeding the tier mask through
   Seam A/B rather than through `DerivedRestRegions`. **This is the single
   change that decides whether the config-only cascade is usable at all on
   R0.5/R1.0 tapers.**
3. **A cap policy for `MAX_REST_REGIONS`.** 64 per band
   (`region_mask.rs:27`, `:182-204`) will silently truncate a terrain tier map.
   Either raise it behind a parameter, or — better, and consistent with the
   repo's own reasoning — make the *conditioning* guarantee the count and treat
   a breach as a refusal rather than a truncation. Note the deliberate absence
   of an area floor and why (`region_mask.rs:45-48`).
4. **A tier-map raster dump**, per §3.6 and the repo's own never-gate-on-an-
   aggregate-without-rendering-the-surface rule. Lift the PGM writer from
   `reference_plate_contract.rs:1119-1175`, or extend
   `planned_regions_to_svg` (`finish_planner.rs:978`).

**Should be measured before anything is built:** `route_greedy`'s O(n²)
(`unified_finish.rs:2448-2475`) and the per-region scallop/waterline surface
rebuild (`unified_finish.rs:1725-1732`) are both explicitly conditioned on
"O(1) regions per band on wanaka today". A tier map is the event they were
flagged against.

### 5.4 Boundary and overlap mechanics — the recipe available today

Config-only, MCP-reachable, no new code:

1. Analysis-carrier toolpath holding the **fine** tool, `rest_analysis.enabled`,
   `reference_tool_id` = the coarse tool — subject to the stock-priority trap at
   `compute/execute.rs:3110-3126`. Tune `cell_mm` / `min_valley_depth` /
   `region_margin_mm` (`config.rs:1527-1535`).
2. Fine `unified_finish` op: `BoundarySource::DerivedRestRegions {
   source_toolpath_id }` pointing at (1) — auto-enables the producer
   (`session/mutation.rs:508-540`).
3. Overlap band = `region_margin_mm` (§4.1 item 4) + `BoundaryConfig::offset`
   (positive = expand, `config.rs:1487`) + optionally
   `BoundaryContainment::Outside` for a further tool-radius
   (`boundary.rs:20-21`).
4. Hold `stock_to_leave` **equal across tiers**, ideally 0, per §4.3.
5. Coarse op takes the complement — no inverse-boundary source exists, so this
   is either `Stock`/`ModelSilhouette` with acceptance of double-cutting inside
   the fine islands (which the repo already tolerates for crease claims:
   overlapping "costs a little double-cutting … and can never abandon
   territory", `unified_finish.rs:1659-1661`), or a keep-out set.

**The honest gap** is not the island machinery. It is two things: today's only
pre-`decompose` tier criterion is **rest depth** (§2.5) where the operator asked
for a **slope/feature** tier; and the one shipped path that reaches the
pre-`decompose` mask dilates its islands by the **shank** radius, which on
R0.5/R1.0 tapers is a measured island-welding failure. Closing those = items 1
and 2 of §5.3, delivered into Seam A or Seam B. Everything else on the list is
already load-bearing, sentried, shipped code.

**Suggested first experiment (no code):** run the §5.4 cascade as-is on
wanaka200 with the R1.0 taper and read `DecomposeStats`
(`finish_planner.rs:280-307`) plus the `TooManyIslands` / `SingleGiantRegion`
classifier (`rest_field.rs:1123-1140`). The predicted result is
`SingleGiantRegion` from the 3.5 mm dilation, not `TooManyIslands`. Confirming
*which* pathology appears settles whether item 2 or item 3 is the real
blocker — and it costs one generation, not a build.

---

## Appendix — key sentries and fixtures

- `crates/rs_cam_core/tests/band_run_off_reproduction_d16_1.rs` — the
  `overlap_mm` run-off reproduction + control (§4.2)
- `finish_planner_wanaka_decompose.rs` — the O(10)-not-O(100) acceptance target
  (`finish_planner.rs:40`)
- `crates/rs_cam_core/tests/classification_columns_ab_m3.rs` — COLUMNS A/B
  harness (§1.2)
- `crates/rs_cam_core/tests/strategy_comparison_h4.rs` — strategy-arm COLUMNS
  scoreboard
- `crates/rs_cam_core/tests/reference_plate_contract.rs:1119-1199` — the PGM
  raster dumper (§3.6)
- `region_mask.rs:345-399` — `region_polygons_caps_at_max_rest_regions_keeping_largest`,
  the cap's own sentry
- `crates/rs_cam_core/src/session/mutation.rs:1445`, `:1472` — the
  `DerivedRestRegions` auto-enable sentries
