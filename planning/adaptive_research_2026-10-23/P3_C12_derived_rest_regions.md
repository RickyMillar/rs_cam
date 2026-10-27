# P3 C12 — D44: the 3D Rough publishes the regions it leaves, and the edge rest op plans only there

Agent C12, 2026-10-27. Design only; no code is changed. The file:line
citations are at `2039cf275` (`.claude/worktrees/adaptive-head`).

The problem (P15 D41, D42b):
- Op 5 ("3D Rough 1 (edges, CP rest)") is a CP rest over the whole part.
  At s1.2 / s2.1 it costs 608 / 763 s.
- Of that, 65 / 78 s of cut removes stock. Another 65 / 72 s cuts beside
  the stock that it then removes.
- 118 / 102 s cuts over at most 0.8 mm of stock: laps of the inner case
  wall and loops in about 30 switch holes.
- 205 / 309 s is rapids: hops between the holes, and a raster across a
  corner.
- No job option limits op 5 (D42b).

## 1. How `DerivedRestRegions` flows today

**The variant.** `BoundarySource::DerivedRestRegions { source_toolpath_id }`
(`compute/config.rs:683-692`) holds only the recipe: the stable id of the
source toolpath. The polygons are read again from the source's cached
result at each generation; they are never copied into the config.

**Storage, on the producer side.** `Produced::rest_regions:
Option<Arc<Vec<Polygon2>>>` (`trace/produced.rs:42-47`). These are XY
polygons in the emission frame, with no Z. `Produced::translated` re-frames
them. Two producers fill it:
- the pencil RestDepth detector, through `detect_rest`
  (`finish/rest_detector.rs:120-148`; `rest_regions: Some(..)` at :138);
- the generic post-generation rest analysis,
  `attach_generic_rest_analysis` (`compute/execute/dressup_apply.rs:105`,
  writing at :181-182).
  - It runs in `compute/execute.rs:717-735` only when all of these hold:
    1. `ctx.rest_analysis.enabled`;
    2. `generated.produced.has_no_rest_output()` (`trace/produced.rs:65`:
       rest_grid and rest_regions both `None`);
    3. a mesh is present.
  - It detects rest valleys of the mesh against a reference tool. That is a
    finishing question ("where can this tool not reach"), not "what did the
    rough leave".

**The demand flag.** A producer's rest analysis is on exactly when an
enabled `DerivedRestRegions` boundary names it:
- `auto_enable_rest_analysis_for_source` (`session/mutation/config.rs:266`)
  sets it on every edit;
- `normalise_rest_analysis_demand` (`session/project_file.rs:1649`) sets it
  on every load.
- Pencil RestDepth is the one exception, because it attaches its own regions.

**Op order and the consumer.**
- **Precondition:** `generate_toolpath` resolves the source before any
  geometry work (`session/compute.rs:2488-2500`, calling
  `resolve_derived_rest_region_polys`, `session/compute/generation.rs:479-535`).
  It refuses with a named message when any of these holds:
  - the source id is missing;
  - the source is the op itself;
  - the source has no result yet;
  - the source has no or empty `rest_regions`.

  So the source op must be generated first. That is the job order (op 1
  before op 5).
- **The pre-clip:** `resolve_generation_inputs`
  (`session/compute/generation.rs:384-430`) takes the regions through
  `RegionSet::processed(keep_outs, boundary_config.offset)`.
  - The result goes to `pre_boundary_regions`. It goes to `pre_boundary`
    (the 3D Rough planner's single outline) only when the set unions to one
    polygon.
  - With several disjoint regions, the 3D Rough plans without an outline
    clip, and the session clips after generation.
- **The post-clip:** `apply_boundary_clip_multi` on the same processed set
  (`session/compute.rs:1027-1050`). The 3D Rough also gets the set as
  `centre_boundary` (`compute/execute/rough_3d.rs:113`; G-BOUNDARYPHANTOM:
  it stamps only what the clip keeps).
- **An empty result:** `compute/generated_empty.rs:105-119` classes an empty
  toolpath under a `DerivedRestRegions` boundary as legitimate.

## 2. What op 1 publishes

**Which pieces.** The research arm already records every piece it leaves,
as `ClearZLevelContext::research_left_pieces` (`adaptive3d/clearing.rs:470`;
filled by `research_leave_piece`, `clearing.rs:3262`). It has two callers:
- the F-038 forecast skip, `clearing.rs:4146` (`why="F-038 forecast"`);
- the "no field" return in `plan_one_outline`, `clearing.rs:3339`.

The waterline cleanup skips contours inside them (`clearing.rs:2429`). On
Corne those pieces are the outer-strip regions at Z 11 / 5 / 0.5 (§16.6
P9), and any F-038 micro-region.

**The form: one XY union over the levels, grown by one planner cell.**
- **Why a union:** `DerivedRestRegions` is XY only. A per-level set needs a
  Z-aware boundary, which no consumer has. The union over-covers only in XY.
  At a level where op 1 left nothing, op 5's planner finds no material
  cell, so its region is empty and it cuts nothing there. Op 5's regions
  come from the material grid on the remaining stock
  (`detect_and_order_regions`; D41 §4).
- **Why grow by one cell:** a left piece is a raster outline of the
  planner's region cells (cell = R / 6, `adaptive3d/path.rs:708`; 0.5 mm on
  Corne op 1). It can lie up to one cell inside the true region. The
  producer grows the union by that one cell, because only the producer
  knows its raster. The consumer then needs no special offset:
  `containment = "center"`, `offset = 0`.
- **Why "center" containment:** the region cells are cells with standing
  material whose centre the boundary allows (`restrict_regions`). A tool
  centre inside the region reaches everything that op 1 left there, plus up
  to R beyond it. The 3D planner never gouges the part, because it drapes
  on the mesh. So op 5 reaches all of op 1's leftovers.

**Where the publish happens.**
- `adaptive3d/mod.rs:1425` builds `Produced`. Set
  `rest_regions: Some(union)` there when the run recorded at least one
  left piece and the op's rest-analysis demand is on.
- The demand flag reaches the planner as a new `Adaptive3dParams` field,
  set by `rough_3d.rs` from `ctx.rest_analysis`.
- **Why demand-gated:** with no consumer, nothing is published and nothing
  changes (§4).
- **One change to the generic analysis:** in
  `attach_generic_rest_analysis`, keep a producer's own `rest_regions` and
  attach only the heatmap grid. Today `has_no_rest_output()` skips the whole
  analysis when either field is set.
  - The precedence becomes per field: a 3D Rough's regions are its
    leftovers, and the GUI heatmap stays.
  - Pencil is unchanged, because it sets both fields.

**The copy op and the holes.**
- The copy op ("3D Rough 1 (copy)", 3 mm, `from_remaining_stock`, its own
  boundary silhouette + 3 mm) does not change. It runs after op 5 and sees
  what op 5 left.
- A switch hole too small for the 6 mm tool is never a left piece of op 1
  that op 5 can cut. If F-038 skipped it, it is in the union, but op 5's
  planner (same R) cannot fit there and plans nothing. The 3 mm copy op
  takes it, as today.
- The holes where op 1 left at most 0.8 mm are not left pieces. Op 5 then
  no longer loops in them. That removes D41's 48–50 s of air loops and most
  of the hops between holes.

## 3. Generic: who else gains

- **Every 3D Rough strategy.** F-038 is shipped code. AgentSearch (and any
  strategy behind the slice dispatch) skips micro-regions there today and
  leaves them with no record. The left-piece record moves out of the
  research gate into the shared `ClearZLevelContext` (a field without
  `research` in its name), filled at the same two call sites. The waterline
  skip stays research-only until a parity-checked step adopts it.
- **Any following rest op:** a 3D Rough on `from_remaining_stock`, or a
  pocket. A `DerivedRestRegions` boundary on any op that takes boundaries
  confines it to the leftovers of a named rough. Example: the multitool
  plan's rest rough with a smaller tool, confined to what the big rough
  could not plan.
- **A later step (not D44):** the planner's own final dexel stock can give
  the true uncleared area per level for every strategy, not only the pieces
  it chose to leave. That needs a threshold above the terrace steps that a
  Z-level rough leaves by design (up to Depth/Pass on a slope). It is out of
  scope here.

## 4. Parity

- With no enabled `DerivedRestRegions` boundary naming a 3D Rough, its
  demand flag is off. It publishes nothing, `Produced` is unchanged, and the
  generic analysis does not run. The G-code and every result stay
  byte-identical.
- With a consumer, only the consumer's toolpath and the producer's
  `rest_regions` / `rest_grid` change. The producer's moves do not change:
  the record is made where the plan already skips.
- The per-field precedence in `attach_generic_rest_analysis` changes
  nothing for pencil (it sets both fields) or for an op that sets neither.
- No serialized surface changes: the config variant, the MCP wire surface
  (`rs_cam_viz/tests/snapshots/mcp_wire_surface.json`) and the GUI boundary
  selector already offer "Rest Regions" with a source toolpath.

## 5. Sentries and the D-run

**Sentries (new):**
1. `a_rough_publishes_the_regions_it_leaves_d44`:
   - On the C12 channel fixture (research arm), with the demand on,
     `Produced::rest_regions` holds one polygon that covers the channel at
     both levels.
   - With the demand off, it is `None`.
   - On `pocket_with_island` (no left piece), it is `None` with the demand
     on.
2. `a_derived_rest_rough_cuts_only_the_leftovers_d44` (session level):
   - op 1 is a 3D Rough with a left piece, and op 2 is a CP rest with
     boundary `DerivedRestRegions(op 1)`;
   - every cutting move of op 2 has its centre within the published union;
   - op 2 is not empty.
3. `the_generic_rest_analysis_keeps_a_producers_regions_d44`:
   - a producer with its own regions keeps them, and gains the heatmap
     grid;
   - pencil RestDepth is unchanged.

**Existing sentries to keep green:**
- `the_planner_is_one_command_f040`;
- `rest_region_tip_radius_dilation_f1`;
- the session `DerivedRestRegions` tests (`session/compute/tests.rs:2281`);
- `adaptive3d_emission_byte_parity`;
- `adaptive3d_boundary_clear_parity`;
- the C12 file.

**The D-run (D45, Corne, as fix29):**
- The job is `phase3/fix27/jobs/k_body_cp_edges_s1p2.toml` with op 5's
  boundary changed. Its source today is `"model_silhouette"`, `"center"`,
  offset 6.0. The new boundary is `containment = "center"`, `offset = 0.0`,
  and the source `[setups.toolpaths.boundary.source.derived_rest_regions]`
  with `source_toolpath_id = 1`. `BoundarySource` is externally tagged
  snake_case (`compute/config.rs:644-646`), so a struct variant is a
  sub-table. The file is saved under a new name; the old file does not
  change.
- The s2.1 job gets the same change.

**Expected op 5 (accel clock):**
- Cut time that removes stock: 65–78 s, as D41 (the same band). The band
  area left after op 5 stays 0 in 3–6.2 mm, and at most 193 / 18 mm² in the
  0–3 / 6.2–9 mm rings.
- Air beside the stock it cuts: about as D41 (65–72 s), because the laps
  around the band remain.
- Air over at most 0.8 mm (inner case wall, holes): 102–118 s → **at most
  15 s**, because those places are outside the union.
- Rapids: 205–309 s → **at most 80 s**. The hops between some 30 holes go;
  the hops between about 4 strip pieces per level remain.
- **Op 5 total: 608–763 s → 200–300 s.** Falsifier: above 350 s at s1.2,
  or any band ring left after op 5 that is larger than in D41.
- The total cycle (D41: 4344 / 3543 s) falls by about op 5's saving. The
  copy op may rise a little, because it now meets in the holes and on the
  case wall what op 5 used to touch. Count it.
