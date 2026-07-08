# Finishing-stack review & cleanup tracker — 2026-07-05

> **How to use this doc**: work top-down within a phase; tick `[x]` as items land.
> Each item lists file:line evidence (verified 2026-07-05 unless marked *agent-reported*).
> Line numbers will drift as fixes land — re-`rg` before editing.
> Phases: P0 = verified cutting-into-part bugs → P1 = duplication-reduction refactors
> (the force multiplier) → P2 = selective-finishing feature → P3 = opportunistic tidiness.
> §R holds adjacent-area research findings (Opus agent sweep).

**Context**: three-agent review of the 14 finishing modules (~12k lines: pencil, rest_field,
rest, crest_lines, scallop, scallop_math, dropcutter, contour_extract, steep_shallow,
waterline, horizontal_finish, radial_finish, spiral_finish, ramp_finish) + hand
verification of the critical claims. Product goal driving priorities: **coarse-tool
rough-finish → fine-tool detail only where it adds detail** on freeform relief parts.

---

## P0 — Verified correctness bugs (fix first, small diffs)

- [x] **P0.1 `stock_to_leave` sign inverted in radial + spiral finish** — VERIFIED
  - `radial_finish.rs:82` and `spiral_finish.rs:129` do `cl.z - params.stock_to_leave`
    → tool cuts INTO the part by the leave amount. `horizontal_finish.rs:228` (`+`) is correct.
  - Tests pin the WRONG sign: `radial_finish.rs:347-376`, `spiral_finish.rs:445-486` — fix both.
  - While here, document the intended semantic once (positive = material remains / tool higher).
    Full cross-op semantics audit is §R2.
- [x] **P0.2 Radial finish 1000 mm dive on interior gaps** — VERIFIED
  - `radial_finish.rs:61` sentinel `bbox.min.z - 1000.0`; `trim_uncontacted`
    (`radial_finish.rs:140-155`) only trims prefix/suffix — interior uncontacted points
    (hole/notch mid-spoke) keep the sentinel and get FEED moves. Machine-crash class.
  - Fix: split spokes into contiguous contacted runs (see P1.1 primitive) — its own doc
    comment ("Keep the longest contiguous run") already describes the right behavior.
- [x] **P0.3 Scallop variable-stepover curvature sign inverted** — VERIFIED
  - `slope.rs` heightmap convention: physical convex → NEGATIVE kappa (pinned by
    `test_curvature_convex`, `slope.rs:579-593`). `scallop_math.rs:71,100` expects
    positive = convex. `scallop.rs:123` passes raw → stepover WIDENS on crests,
    tightens in coves — exactly backwards.
  - Fix: negate at the `scallop.rs:123` call site + add a cross-module convention-pinning test.
- [x] **P0.4 steep_shallow chords across excluded regions at constant Z** — gouge class
  - `steep_shallow.rs:175-219`: steep contour points filtered by mask, survivors connected
    sequentially, loop force-closed back to `filtered[0]` — chords cross shallow regions
    at cutting feed where surface may be above Z.
  - Correct contiguous-run pattern already exists: `ramp_finish.rs::slope_confined_segments:295`.
    Fix via P1.1 primitive (or interim: reuse slope_confined_segments pattern).
  - Same disease, milder: spiral gap chords (`spiral_finish.rs:126-134,177-194`), scallop
    slope-filter chords (`scallop.rs:463-470,494-510,534`).
- [x] **P0.5 Pencil off-mesh guard is dead code**
  - `pencil.rs:1567-1573` filters `p.z > NEG_INFINITY + 1.0` but `lift_to_surface`
    (`pencil.rs:707-732`) never produces the sentinel — off-mesh (bisector-shifted) points
    pass through with raw interpolated Z and no stock_to_leave.
  - Fix: `lift_to_surface` marks non-contact (Option/NaN), emit loop SPLITS paths there;
    delete the dead filter.
- [x] **P0.6 rest_grid heatmap overlay not display-shifted** (uncommitted WIP, fold into overlay task)
  - `rs_cam_viz/src/app/gpu_upload.rs:997-1008` `translate_annotated` shifts toolpath moves
    by F-028 display_shift but clones `rest_grid` with untranslated `origin_x/origin_y`
    → heatmap misaligned on origin≠0 projects; non-identity setups also unmapped.
  - Fix when building the Layer-3 render: apply the same shift to grid origin (and decide
    setup-remap story). See §R3 for the general side-channel-payload audit.
- [x] **P0.7 horizontal_finish double-machines overlapping flat regions**
  - `horizontal_finish.rs:161-166 + 178-255`: per-Z-region raster uses each region's
    cutter-expanded bbox but the GLOBAL flat test — two flat regions at different Z with
    overlapping XY bboxes each machine the other's flats (duplicate cuts, wrong-Z first pass).
  - Consider whether the op survives at all: the DropCutter adapter already supports a slope
    window (`execute.rs:1496-1510`) — horizontal's only unique value is per-Z ordering (P3.6).

- [x] **P0.8 Seven mesh-finish ops are UNCANCELLABLE** — VERIFIED (the 2-hour-hang class)
  - scallop, pencil, ramp_finish, steep_shallow, radial_finish, spiral_finish,
    horizontal_finish all accept `ctx.cancel` and never poll it; they build dense
    heightmaps via non-cancel `SurfaceHeightmap::from_mesh` (verified `scallop.rs:326`,
    `ramp_finish.rs:368`, `steep_shallow.rs:340`) while `from_mesh_with_cancel`
    (`slope.rs:71`) sits unused. Only adaptive2d/adaptive3d/drop_cutter/waterline poll
    (`execute.rs:797,1017,1468,1562`), and the cancel sentry test covers only those 4.
  - Fix: swap to `_with_cancel` heightmap + per-row cancel checks in sample loops
    (natural home: P1.2 FinishContext); extend the sentry to all families.
  - Also fix the LYING doc at `execute.rs:187-190` claiming adapters "can't silently drop
    the cancellation closure".
- [x] **P0.9 adaptive3d collapses radial+axial stock_to_leave to `max()`** — VERIFIED
  - `execute.rs:932`: `stock_to_leave: cfg.stock_to_leave_axial.max(cfg.stock_to_leave_radial)`.
    Two config fields, UI dials for both, only the max reaches the planner — radial=0.5 +
    axial=0.0 leaves an unexpected 0.5 mm floor. Either plumb both through the planner or
    collapse the config honestly to one field.

## P1 — Duplication-reduction refactors (the force multiplier)

> These are ordered so each one structurally prevents a P0 bug class from recurring.
> Rule of thumb going forward: **no finish op implements its own sampling, masking,
> run-splitting, or emission** — they compose shared primitives.

- [x] **P1.1 Shared `mask → contiguous runs → emit` primitive**
  - Generalize `ramp_finish.rs::slope_confined_segments:295-322` to any point predicate;
    emit runs via the canonical `Toolpath::emit_path_segment_with_intent` (`toolpath.rs:211`).
  - Retires THREE masked-raster implementations: `steep_shallow.rs:249-311`,
    `horizontal_finish.rs:186-255`, `raster_toolpath_from_grid_with_slope_filter`
    (`toolpath.rs:530-560` / execute adapter `execute.rs:1502`).
  - Structurally fixes P0.2 / P0.4 and the spiral/scallop chord variants.
- [x] **P1.2 Shared finish-op setup (`FinishContext`)**
  - Heightmap + SlopeMap construction block is verbatim ×3: `steep_shallow.rs:331-343`,
    `ramp_finish.rs:357-371`, `scallop.rs:317-329` (+ test copies `scallop.rs:616-627,660-670`).
  - Fold in: named constants for the slope-window sentinels `0.01/89.99`
    (`scallop.rs:417`, `ramp_finish.rs:414`, `execute.rs:1496`); ONE Z-ladder helper with one
    epsilon (currently `waterline.rs:132` 1e-10 vs `steep_shallow.rs:149` 0.01 vs
    `ramp_finish.rs:381` z_step·0.5).
- [x] **P1.3 Unify the drop-cutter GRID layer**
  - Point layer is healthy (`point_drop_cutter` ~12 consumers, no copy-paste). Grid layer
    exists ×3: `DropCutterGrid` (dropcutter.rs), `SurfaceHeightmap::from_mesh_with_cancel`
    (`slope.rs:84-122`, reimplements the rows×cols loop), rest_field's own sampling grid —
    each with its own cancellation cadence and min_z semantics.
  - Retire `compute_grid_slopes` (`dropcutter.rs:259-297`, degrees, one consumer
    `execute.rs:1501`) in favor of `SlopeMap` (radians + normals + curvature).
  - Latent trap: `DropCutterGrid.x_start/y_start` hold ROTATED-frame mins under
    world-sounding names (`dropcutter.rs:171-176`) and `direction_deg` isn't stored;
    `steep_shallow.rs:262-263` recomputes world coords from them (safe only because always 0°).
    Rename fields or store the angle.
- [x] **P1.4 One closed-contour emitter + waterline cancel-path dedup**
  - `waterline.rs:146-179` ≡ `steep_shallow.rs:194-219` closed-loop emission — extract
    `emit_closed_contour` (this is also where climb/conventional orientation should live ONCE;
    today ramp has naive segment-reversal `ramp_finish.rs:498-514`, waterline has nothing).
  - `waterline_contours` (`waterline.rs:36-84`) vs `_with_cancel` (`:188-224`): ~45 dup lines
    AND the cancellable (GUI) variant lost the rayon parallel join (`:220-221`) — make the
    plain fn a thin wrapper over the cancel variant, restore `rayon::join`.
- [x] **P1.5 pencil.rs structural split**
  - Extract the dihedral detector (~`pencil.rs:230-703`) into its own module mirroring
    crest_lines/rest_field; each detector arm returns `Vec<Vec<P3>>` centrelines.
  - Split the 455-line `pencil_toolpath_structured_annotated` (`pencil.rs:1209-1663`);
    hoist ONE reference-resolution fn (currently 3 shapes: `:1237-1245`, `:1325-1383`,
    test helper `:2026-2032`).
  - Merge `chain_passes_depth` (`:774-811`) / `polyline_passes_depth` (`:1170-1207`)
    (identical median-of-≤8 gate); collapse `gate_chains_by_depth` parallel/serial dup bodies
    (`:870-899`).
  - Unify rest-gate semantics across detectors (Curvature skips gate when
    `min_valley_depth == 0` `pencil.rs:1264`; Dihedral always gates; RestDepth bakes it in —
    three meanings for one user dial).
  - `impl Default for PencilParams` (kills 6× 18-field test literals: `:1863,1898,2044,2111,2465,2540`).
- [x] **P1.6 rest_field.rs `Grid<T>`**
  - Nine index-laundering accessors (`rest_field.rs:535-613`; `get_f64`≡`get_f64_vec`,
    `get_usize`≡`get_usize_vec` byte-identical) → small `Grid<T> { nx, ny, data }` with
    `get/set/at(r,c)`; centralizes row-major math re-derived at ~10 sites.
  - Split the 320-line `detect_rest_valleys` (`:209-528`) into its 7 phases.
- [x] **P1.7 contour_extract.rs split**
  - Three jobs in one file: fiber waterline weaving (`:28-372`, waterline-only), bool-grid
    marching squares (`:374-612`), EDT/curvature/box-blur scalar-field math (`:614-813`).
    Move the field math to a `grid_field` module.
  - Collapse the two ~90-line chainers (`chain_segments:282-372` vs `chain_segments_2d:524-612`);
    port fiber marching squares onto the `MS_CASES` table with ONE corner convention
    (currently opposite row orientations: `:109` vs `:457`).
  - Fix lying saddle-disambiguation comment (`:147-149` — no center test is performed).
- [x] **P1.8 Small dedups (batch into any nearby PR)**
  - `polyline_length` ×3: `pencil.rs:1051`, `rest_field.rs:615`, `crest_lines.rs:761`.
  - `EdgeKey` ×2: `pencil.rs:208-215`, `crest_lines.rs:87-93`.
  - bbox-center max-corner radius: `radial_finish.rs::compute_max_radius:121-135`
    ≡ `spiral_finish.rs::corner_distance:221-236`.
  - Annotated-wrapper boilerplate: `spiral_finish.rs:93-100,206-216` ≡ `ramp_finish.rs:339-346,557-567`.
  - Probe-ball constants (0.1mm/10mm) ×2: `pencil.rs:1347`, `rest_field.rs:1162`;
    nominal reference ball length 25.0 ×2: `pencil.rs:1243,2027`.
  - Test hillshade harness ~150 lines ×2: `rest_field.rs:1139-1289` ≈ `crest_lines.rs:938-1040`
    → shared `#[cfg(test)]` util.
  - `sample_chain` (`pencil.rs:487-535`) is `#[allow(dead_code)]` kept alive by one test —
    fold into the one polyline-walker (3 impls: `sample_chain`, `sample_chain_bisected`,
    `resample_polyline`).
  - `chain_contours_pub` (`waterline.rs:232`) pointless pub wrapper.
  - `generate_spiral_points` (`spiral_finish.rs:245-251`) test-only w/ cfg_attr — move into test mod.
  - Rename `rest.rs` → `rest2d.rs` (or reciprocal module-doc cross-links) — zero code overlap
    with rest_field.rs but the naming invites confusion.

## P2 — Selective finishing: derived rest-region boundaries (the product goal)

> Goal: Ø6 ball rough-finishes everything fast → rest analysis lights up detail regions
> → fine tool (scallop/pencil) runs ONLY inside those regions.
> All building blocks exist; this phase wires them.

- [x] **P2.1 Region polygons from the rest field** — DONE 2026-07-06 (pending verify):
  `region_polygons_from_mask` in rest_field.rs (EDT dilate → `marching_squares_bool_grid` →
  degenerate-loop drop → `detect_containment` → `ensure_winding`); `RestFieldResult.region_polygons`
  dilated by `pencil.radius() + params.region_margin_mm` (new param, default 0.5);
  `AnnotatedToolpath.rest_regions: Option<Arc<Vec<Polygon2>>>` shifted by `translated()`,
  plumbed pencil→execute, pass-through in all dressup-family transforms + session/compute.
  - `ClearingRegion` today = bbox + cell count + peak depth (`rest_field.rs:107-115`),
    consumed by NOTHING (only `info!` logging `pencil.rs:1415-1425`). The doc comment at
    `rest_field.rs:105` already names the plan ("Phase D → adaptive3d FromRemainingStock
    boundaries") — never built.
  - Build: rest mask components → `marching_squares_bool_grid` (`contour_extract.rs:374`,
    already returns closed world-coord loops, already used as machining boundaries by
    `adaptive3d/clearing.rs:678,1049,1432` + `adaptive/spiral.rs:319`) → dilate by fine-tool
    radius + margin (use EDT `distance_transform_2d` + threshold, NOT `dilate_grid`'s
    O(cells×radius) loop) → `Vec<Polygon2>` stored beside `rest_grid` on the generated toolpath.
  - Caveats to handle: marching-squares output has no winding-order normalization or
    outer-vs-hole nesting classification; `Polygon2` boundary clip uses exterior only.
- [x] **P2.2 New derived boundary source** — DONE 2026-07-06:
  `BoundarySource::DerivedRestRegions { source_toolpath_id }` (compute/config.rs); fail-hard
  staleness first-thing in `generate_toolpath` (missing id / self-ref / ungenerated source /
  no regions — each names the fix); multi-region set-clip
  `boundary::clip_toolpath_to_boundary_set_with_provenance` is now the SOLE clip walk
  (single-polygon fn delegates to it); `apply_boundary_clip_multi` routes the variant,
  per-region keepouts+offset via `resolve_derived_region_polygons`; adaptive3d pre-clip
  uses `union_all` when it yields one polygon, else post-clip-only. GUI: boundary picker
  "Rest Regions" + ready-first source combo, boundary edits now mark toolpath stale
  (pre-existing gap closed), MCP set (`derived_rest_regions` + `source_toolpath_id`) +
  boundary in `get_toolpath_params`, project-IO round-trip test.
  - `BoundarySource` (`compute/config.rs:243`) has only Stock / ModelSilhouette /
    Geometry{imported} / FaceSelection — no computed variant.
  - Add e.g. `DerivedRestRegions { source_toolpath_id }` (or a session store for derived
    polygon sets); resolve in `apply_boundary_clip` (`session/compute.rs:1443`).
    Precondition/staleness handling mirrors the FromRemainingStock fail-hard pattern
    (`session/compute.rs:1158-1186`).
- [x] **P2.3 Thread boundary regions into the finish family (pre-clip)** — DONE 2026-07-06:
  `ExecutionContext.boundary_regions: Option<&[Polygon2]>` (sibling of adaptive3d's
  `boundary`), resolved ONCE in `resolve_generation_inputs` and shared with the post-clip;
  all 7 finish ops take `Option<&[Polygon2]>` on their `_with_cancel` entry (None =
  byte-identical): radial/spiral/horizontal skip the drop-cutter query pre-sample,
  steep_shallow folds into its masks, ramp into its slope-filter split, waterline clips
  closed contours via `split_runs(Closed)` (partial loops emit as open segments, no chords);
  scallop scans per-region + BONUS: `ring_to_3d` now gates on `covered ∧ finite` (off-footprint
  dive fixed) and the continuous-mode ring connector is a guarded helical link (≤3R, kept
  anchor) — fixed a P0.4-class chord regression caught by the sentry AND a latent
  multi-region cutting bridge. Per-op confinement + no-op pin tests added.
  - `ExecutionContext.boundary` (`execute.rs:210,1643-1651`) is consumed ONLY by the
    adaptive3d family. None of the six directional ops accept region restriction;
    scallop hardcodes the bbox rectangle (`scallop.rs:331-376`) though
    `generate_scallop_rings` already accepts an arbitrary `Polygon2` (`scallop.rs:167`).
  - Post-clip alone works TODAY (generic `apply_boundary_clip` post-dressup,
    `session/compute.rs:1304-1309`) but generates the full part then discards — at 0.5 mm
    stepover over a 200 mm part that's the difference between seconds and minutes.
  - With P1.1 in place, boundary support for all drop-cutter ops = point-in-polygon predicate
    on the same run-splitter; waterline-family needs contour clipping to the polygon.
  - Bonus fix rolled in: scallop off-footprint corners currently dive to `bbox.min.z`
    (`scallop.rs:140-159` clamps non-contact instead of using `SurfaceHeightmap.covered`).
- [~] **P2.4 GUI/MCP surface + advisor** — overlay DONE 2026-07-06: rest heatmap renders in
  the wgpu viewport (`rest_heatmap_mesh.rs` NaN-skip mesh, threshold-anchored p95 ramp via
  the SAME `rest_ramp_color` the legend draws, depth-read-only height-plane pipeline,
  Show▼ toggle + gradient legend; threshold semantics shared with `region_polygons_from_mask`
  = one source of truth). Advisor + steep/shallow alternative signal REMAIN (below).
  - Advisor (later): suggest "scallop clipped to regions A..N with tool T" from the rest
    report — candidates ranked by `peak_rest_mm` significance.
  - Advisor (later): suggest "scallop clipped to regions A..N with tool T" from the rest
    report — candidates ranked by `peak_rest_mm` significance.
  - Alternative detail signal for parts without a rest reference:
    `classify_steep_shallow` (`slope.rs:405`) + dilate + marching squares = steep/shallow
    region polygons; `SlopeMap::curvature_at_world` as a "high-detail" detector.
- [x] **P2.5 RegionSet consolidation + op-agnostic rest analysis** — DONE 2026-07-07:
  new `region_set.rs` (`RegionSet` newtype over `Vec<Polygon2>`, Cow-backed so hot paths
  borrow instead of clone) replaces 4+ hand-rolled copies of "point-in-any-region",
  "union-and-collapse-to-one", and "per-region keepout+offset": `session/compute.rs`'s
  `resolve_derived_region_polygons` + the `resolve_generation_inputs` union-collapse, the
  GUI worker's two copies in `worker/execute/mod.rs` (fixed a latent divergence in the
  process — the worker's `pre_boundary` union used to union RAW regions then keepout+offset
  the union, while `pre_boundary_regions` did correct per-region processing; both now share
  one `RegionSet::processed(...).single_union()` call), and the 7 mesh-finish ops'
  `regions.iter().any(|r| r.contains_point(p))` closures (`ExecutionContext.boundary_regions`
  + each op's own param is now `Option<&RegionSet>`).
- [x] **P2.5 generic rest analysis** — DONE 2026-07-07: `RestAnalysisConfig` (compute/config.rs;
  `enabled`, `reference_tool_id`, `cell_mm`, `min_valley_depth`, `region_margin_mm`, defaults
  mirroring `RestFieldParams`) on `ToolpathConfig`/`ToolpathEntry` — ANY operation can now
  enable rest analysis against its OWN tool, not just pencil's `RestDepth` detector arm.
  Runs as a shared post-generation step in `execute_operation_annotated_with_regions`
  (new `attach_generic_rest_analysis` helper, reusing `rest_field::detect_rest_valleys` +
  pencil's exact reference-resolution order: machined stock → real reference tool →
  self-referenced probe). Precedence: skipped when the op already attached its own rest
  artifacts (pencil), so there is one source of truth per toolpath. GUI: "Rest Analysis"
  section (sibling of Machining Boundary) with enable checkbox + reference-tool picker +
  cell/threshold/margin fields; threaded through `ComputeRequest` like boundary. MCP:
  `set_rest_analysis_config` mirrors `set_boundary_config`; `rest_analysis` added to
  `get_toolpath_params`. Verified a non-pencil (scallop) source feeds a downstream
  `DerivedRestRegions` boundary end-to-end (worker test).

## P3 — Opportunistic tidiness (do when touching the file anyway)

- [ ] P3.1 scallop: extract 45-line inline rect sampler (`scallop.rs:331-376`) to `polygon.rs`;
  reconcile stepover clamp `0.05R..3.0R` (`:225-226`) vs scallop_math's 4R cap
  (`scallop_math.rs:122`); fix doc bugs (`scallop_math.rs:13` return-value doc,
  `:62` backwards "tool fits" comment, header claiming steep_shallow consumes it);
  module doc oversells "variable stepover" (per-ring averaged scalar, `scallop.rs:200-221`).
- [ ] P3.2 steep_shallow: hardcoded 30° contour-majority threshold (`:146`) vs
  `params.threshold_angle` (`:346`); wall_clearance erosion silently undone by overlap
  dilation with defaults (`:362-367`); param name mismatch `shallow_eroded`/`shallow_expanded`
  (`:236` vs `:403`); doc-rot (header scallop-height claim `:15`, default 40° vs actual 45
  `:11,:30,:61`); masked sampling instead of full-footprint batch + filter (`:246`).
- [ ] P3.3 ramp_finish: seam alignment in `ramp_between_contours` (`:244-289` — t=0 anchors
  arbitrary per contour, no nearest-point re-anchoring; interpolated points never
  drop-cutter re-verified → gouge risk on non-ruled walls); `shape_frac = global_frac`
  (`:281`) blends shape across the whole multi-rev ramp; `match_contours` centroid heuristic
  (`:229`).
- [ ] P3.4 waterline: no stock_to_leave at all; no climb/conventional control;
  O(n²) greedy chaining can self-cross (documented fallback — fine, just noting).
- [ ] P3.5 horizontal_finish: `z_tolerance = stepover/2` coupling (`:118`); quadratic
  drain-and-restart clustering (`:122-157`); per-point queries, no batching, no cancel.
- [ ] P3.6 Consider op consolidation: horizontal_finish ≈ DropCutter+slope-window
  (`execute.rs:1496-1510`) + per-Z ordering; radial & spiral are both "drop-cut a parametric
  XY curve" — one skeleton; steep_shallow → ~50-line composition after P1.1/P1.2/P1.4.
- [ ] P3.7 pencil: `order_paths_nearest` (`pencil.rs:901-988`) O(n²) greedy w/ placeholder-swap
  (crate has `tsp.rs`); rest_field centerline-Z `bbox.min.z` fallback (`rest_field.rs:455-459`)
  dead-ish; `rest.rs:94` `sample_step = tool_radius.clamp(0.25,0.5)` unexplained coupling.
- [ ] P3.8 crest_lines: heightfield assumption (+Z normals) makes Curvature detector wrong for
  undercut/CAD-wall parts — documented in code; surface a GUI note someday.

## §R — Adjacent-area research (Opus agent sweep, 2026-07-05)

> Areas that looked suspect from review evidence but sit OUTSIDE the 14 reviewed files.
> Findings below get promoted into P0/P1/P3 rows as they're confirmed.

### R1 — Shared geometry/emission layer (slope.rs, toolpath.rs, polygon/offset)
*Status: DONE 2026-07-05. Verdict: shared layer is in decent shape to lean on — primitives
capable and well-tested — but two live landmines + consistency gaps a refactor must fix
rather than inherit. No dead code in the six shared files.*

**Correctness (promote-with P0):**
- [x] **R1.1 SlopeMap curvature doc LIES** — `slope.rs:197-198` says "positive = convex" but
  code (`:244`) + test (`:579-593`) produce NEGATIVE for physical convex. Fix doc alongside
  P0.3 and reference the convention from scallop_math.
- [x] **R1.2 Marching-squares saddle divergence** — the TWO MS implementations resolve saddle
  cases (5,10) to OPPOSITE topologies: `boundary.rs:328-335` joins BL/TR;
  `contour_extract.rs:146-172` joins BR/TL, with a stale comment claiming "disambiguate by
  center value" that computes no center value. Contours pinch differently at diagonal
  touchpoints. **Prerequisite for P2.1** (MS regions → boundary clip must share one saddle
  convention). Merge into one shared MS routine [M] — supersedes part of P1.7.

**Emitter layer:**
- [x] **R1.3 Intent-tagging inconsistency in the raster builders** —
  `raster_toolpath_from_grid` (`toolpath.rs:440`) tags intents correctly but HARDCODES
  `MoveIntent::FinishingCut` (`:501,518`); `raster_toolpath_from_grid_with_slope_filter`
  (`:542`) uses plain helpers → every move `MoveIntent::Unknown` (`:613-626`). Downstream
  metrics/gates key off intent. Fix with P1.1 [S].
- R1.4 `emit_path_segment` adoption map: USED by ramp/horizontal/rest/project_curve/vcarve/
  inlay/radial; HAND-ROLLED by pocket, profile, adaptive, waterline, scallop, spiral,
  steep_shallow, zigzag, both raster builders → extends P1.4's scope beyond finishing [M].
  Raw `Move` pushes only in tsp.rs/arcfit.rs (legit, below helper layer).
  LeadIn/LeadOut intents emitted only by dressup layer — correct layering, no gap.

**Polygon2 gaps (feed into P2.1 design):**
- [x] **R1.5 No self-intersection/pinch repair** — offset robustness is a
  `catch_unwind` swallowing cavalier panics (`polygon.rs:169-183`); MS saddle output is
  exactly the pinched-ring shape that triggers it. Guard/repair before offset [M].
  **Landed 2026-07-06**: `Polygon2::has_self_intersection` (segment-pair crossing test,
  bbox-prefiltered) + `Polygon2::repaired` (geo boolean self-union resolves pinched/bowtied
  rings into simple polygons via the even-odd fill rule); `offset_polygon` now repairs
  self-intersecting input before it ever reaches cavalier_contours, with the `catch_unwind`
  kept as a near-unreachable backstop.
- [x] **R1.6 No boolean ops** (union/intersect/difference) — needed to merge/subtract derived
  region polygons; wrap `geo::BooleanOps` [M].
  **Landed 2026-07-06**: `Polygon2::union`/`intersection`/`difference` (pairwise,
  `geo::BooleanOps`) + `Polygon2::union_all` (`geo::unary_union` single-pass merge);
  MultiPolygon output mapped back through `from_geo_polygon` + `ensure_winding`.
- R1.7 Smaller: `detect_containment` nests one level only (`polygon.rs:363`); offset hole
  re-assignment heuristic can misassign on region splits (`:257-273`); `contains_point` has
  no boundary epsilon (`:418-436` — MS output sits exactly on grid-aligned edges); THREE
  ray-cast PIP copies (`polygon.rs:418`, `boundary.rs:436` test-only, `Polygon2::contains_point`)
  → collapse to one [S].
  **PIP-collapse landed 2026-07-06**: `boundary.rs`'s test-only `point_in_ring`/
  `point_in_polygon` duplicates deleted in favor of `Polygon2::contains_point`; canonical
  `polygon::point_in_polygon` made `pub(crate)`; new `Polygon2::contains_point_eps(p, eps)`
  adds the boundary epsilon for MS-aligned points without touching the hot-path
  `contains_point`. Boundary-epsilon and PIP-collapse done; containment-nesting and
  hole-reassignment sub-items remain open.
- R1.8 Offset sign convention: positive = INWARD (`polygon.rs:150-153`, `boundary.rs:37-40`) —
  consistent internally but opposite to typical clipper convention; document loudly.

**Grid census verdict (confirms P1.6 direction, widens scope):**
- R1.9 8 grid types found. `Grid<T>` should serve the 5 scalar/heightmap grids —
  SurfaceHeightmap, SlopeMap, DropCutterGrid, RestGrid, MaterialGrid (all flat row-major
  `row*cols+col`, origin+cell_size; divergences cosmetic: RestGrid `nx/ny/cell_mm` naming,
  DropCutterGrid anisotropic steps; empty-cell policy = trait). Do NOT force in DexelGrid/
  TriDexelStock (interval lists, multi-axis) or adaptive EndpointGrid (sparse hash) [L].
  Conventions otherwise uniform: winding (ext CCW), Z-up, MS corner bit-packing identical.

### R2 — execute.rs adapter layer + cross-op param-semantics audit
*Status: DONE 2026-07-05. Swept all 23 registry adapters. Two findings promoted to
P0.8/P0.9 (verified). Layer shape: exhaustive match at execute.rs:1685-1743 is dead at
runtime but intentional (compile-time exhaustiveness net) — leave it.*

**stock_to_leave census (completes the P0.1 picture):**
- Correct `+`: horizontal (`:228`), scallop (`scallop.rs:152/154`), pencil (`:717/1023`).
- INVERTED `−`: radial, spiral (= P0.1).
- Sign-ok but WRONG MECHANISM: ramp_finish — uniform +Z shift of the z-ladder bounds
  (`ramp_finish.rs:374/375`), leaves ≈0 material on the steep walls its 30-90° default
  window targets. Silent under-leave. (Elevates P3.3 → do with P0.1.)
- Collapsed: adaptive3d `max(axial, radial)` (= P0.9).
- ABSENT: waterline, drop_cutter (no field at all — "leave 0.2mm" impossible on two finish
  strategies), and all 2D/roughing ops (those leave stock via boundary offset — by design).
- All are Z-only; nobody applies lateral leave except adaptive3d's planner.
- [ ] **R2.1** Add stock_to_leave to waterline + drop_cutter configs [S].
- [ ] **R2.2** Define the single semantic (positive = material remains) in one doc place;
  add a cross-op sign sentry test so drift can't recur.

**Cancellation census:** 4/23 poll, 19/23 ignore (7 mesh-finish = P0.8; the rest are
fast 2D ops — lower risk but the sentry should still cover them).

**Verified clean / no action:** retract & safe-Z (single `effective_safe_z` helper,
config.rs:47, all adapters consistent; no feed-where-should-rapid found anywhere);
slope-window units (deg/rad conversions all correct, windows inclusive both ends —
only the sentinel dup remains, = P1.2); tool geometry (all ops delegate to the
MillingCutter trait, no local ball/taper re-derivations; pencil 25.0 flute length is
one live site, `pencil.rs:1243`).

**Adapter boilerplate consolidations (fold into P1 batches):**
- [x] **R2.3** [M] `family_adapter!` macro for the 23× identical config-guard +
  "registry adapter mismatch" string.
- [x] **R2.4** [S] `require_index(ctx)` helper (~8× duplicated "requires a spatial index").
- [x] **R2.5** [S] `vbit_half_angle(tool)` helper (3× duplicated VBit match:
  `execute.rs:395,454,502`).
- [x] **R2.6** [S] `slope_filter_active(from,to)` + named eps const (kills the
  0.01/89.99 triple — same item as P1.2 bullet).
- [x] **R2.7** [M] fold the ~12× `if let Some(sem) = ctx.semantic_ctx { annotate_depth_run_spans }`
  into `generated_with_depth_run_spans(sem: Option<..>)`.
- R2.8 minor: waterline + face pass `&[]` z-levels to depth-run spans (waterline has real
  levels — span-fidelity gap, do with P1.4).

### R3 — Frame/transform handling (world vs local vs display)
*Status: DONE 2026-07-05. Verdict: the whole bug class reduces to ONE structural gap —
`translate_annotated` (gpu_upload.rs:997) is the only post-generation site that shifts a
toolpath's frame, and it shifts ONLY the moves. Everything else checked out.*

**Confirmed (updates P0.6):**
- [x] **R3.1 `rest_grid` left behind by display shift — CONFIRMED but LATENT** — no viz
  consumer reads `rest_grid` yet, so it cannot manifest today; it is armed for the Layer-3
  overlay. Fix BEFORE the overlay lands.
- [x] **R3.2 `planner_engagement` ALSO left behind by `translate_annotated`** — cut_points
  stay emission-frame on the shifted clone. Harmless today (consumed core-side only, never
  off the display clone) but same defect; would bite any future viewport overlay
  positioned off it.

**Fix plan (agent-recommended, adopt as the P0.6 implementation):**
1. [S] `translate_annotated` transforms the WHOLE AnnotatedToolpath: shift `rest_grid`
   origin_x/y (+ surface_z by shift.z; `Arc::make_mut` on the rare display path) and
   `planner_engagement` cut_points. Ship with the overlay work.
2. [S] Regression sentry mirroring `translate_annotated_shifts_targets_and_preserves_arc_offsets`
   (gpu_upload.rs:1061): non-None rest_grid origin + engagement points shifted by same vector.
3. [M] Promote invariant into core: `AnnotatedToolpath::translated(shift)` in
   toolpath_spans.rs via full destructure — any FUTURE coordinate payload then forces a
   compile-time decision instead of silently inheriting the move-only shift. Durable fix.
4. [S] Doc-comment the frame contract on rest_grid/planner_engagement in toolpath_spans.rs
   ("emission-frame; must be re-framed anywhere toolpath.moves are re-framed").

**Verified clean (no action):**
- Dressups (arcfit/condition/tsp/feedopt/dressup), boundary clip, post-sim feed modulation:
  none shift frames → payload passthrough is CORRECT. Spans are move-index-only (frame-agnostic),
  remapped via provenance on clip. G-code export consumes emission frame as-is.
- **F-024 non-identity exposure: structurally sound.** For flipped/rotated setups the mesh,
  polygons, prior-stock snapshot, dexel grid AND toolpath all share the same zero-rooted
  local frame (eval_context.rs:82-150, compute.rs:999-1017, simulate.rs:408-432) — the
  F-024 mismatch (grid local vs toolpath world) was identity-setup-specific and is
  structurally impossible for non-identity. FromRemainingStock across a flip is
  frame-consistent (rest_field queries the per-setup group_stock in the same local frame).
  Residual risk is display/consumer-side only: any consumer assuming world coords misaligns
  for non-identity setups AND identity-with-nonzero-origin — i.e. R3.1's generalization.

---

## Follow-ups surfaced during implementation (2026-07-05, unassigned)

- [x] **F.1 GUI still shows the inert adaptive3d radial stock-to-leave dial.** The planner is
  a pure Z-offset engine (every use site is `z + stock_to_leave`; no wall-normal mechanism
  exists). P0.9 fix made the adapter honestly axial-only. Decide: grey-out/label the radial
  dial in rs_cam_viz, or build a real XY wall-offset mechanism in the planner (L).
- [ ] **F.4 (2026-07-07) FromRemainingStock regen catch-22 after fresh load.**
  `controller/events/compute.rs:~371` finds the prior-stock checkpoint by locating the
  toolpath ITSELF in the last sim's `boundaries()`; an errored/ungenerated op is never
  simulated → never in boundaries → can never regenerate. Every FromRemainingStock op is
  permanently Error after a fresh project load (wanaka Rivers/Lakes/3D Rough 6/3D Finish 6).
  Fix: key the checkpoint on the op's position in the PLANNED sim order, not membership in
  the last simulated set. Blocks R2 machined-stock pencil sim validation. [M]
- [ ] **F.5b (2026-07-07) screenshot_toolpath `include_rapids:false` still draws
  rapid-over moves** — they carry `MoveIntent::Linking`, and the filter tests intent, not
  move type; long straight safe-Z lines clutter cutting-only exports. Filter on move TYPE
  (or intent ∈ {Linking, Retract} when the move is a rapid). [S]
- [ ] **F.6 (2026-07-07) `export_gcode` ignores `accept_unmodeled=true`** — still refuses
  on SimulationRequired criteria with the exact same message. [S]
- [ ] **F.2 Pencil detector internals are not cancel-aware.** P0.8 added phase-boundary +
  per-path polls in pencil.rs, but `crest_lines::detect_valley_lines`,
  `rest_field::detect_rest_valleys`, and the Dihedral chain builders have long internal
  loops with no polling — if a detector becomes the multi-minute long pole, thread
  `_with_cancel` into them.
- [x] **F.3 Pre-existing red root cause found (not from this batch):**
  `all_operation_families_emit_expected_structural_span_kinds` fails on Pencil at HEAD too;
  the `make_v_groove_mesh` fixture in execute.rs (~2363) has inverted winding (all four
  normals point down; hand-computed). pencil.rs's own test module fixed its copy of the
  same fixture with a comment. Fix the execute.rs fixture winding and see if the red clears.
- Other 3 pre-existing reds (unchanged at HEAD, known): 2× adaptive3d peck/rapid tests
  (accel-friendly work), 1× planner_sim_dexel_parity_agent_search (wanaka Back Rough anomaly).
- [x] **wanaka_suggest_baseline re-baselined 2026-07-06.** Stale since the 2026-06-20
  unified-load-model recalibration (Ks=49.95/F_edge=5.30 anchored to GenericHardwood) made
  the closed-form deflection back-off long/thin-tool-only — Back Rough / 3D Rough 6's 6 mm
  carbide stub is chipload/power-bound, so the vendor_ap axial-DOC envelope now clamps DPP
  (9.0 mm → ~5.4 mm) before the deflection solve ever runs, and the old
  `DppCappedByDeflection` (~3.69 mm) chain never fires. Rewrote the Back Rough / 3D Rough 6
  assertion blocks in `wanaka_suggest_integration.rs` to check
  `AxialDocClampedByEnvelope { binding: "vendor_ap" }` + a 5.4 mm determinism pin, and added
  an explicit "`DppCappedByDeflection` must NOT fire" guard. All 3 tests in the file green.

## §S — Round-2 sweep: dressup pipeline, 2D ops, session/compute, feeds (2026-07-06)

> Opus research sweep over the NOT-previously-audited areas. All claims verified against
> the working tree by the sweep. Verdicts: feedopt.rs MESS; dressup/arcfit/condition,
> pocket/profile/zigzag/trace/vcarve/inlay, adaptive/mod, session/compute needs-work;
> tsp, feed_modulation, compute/spans, adaptive/{path,search,spiral,material_grid},
> feeds force/geometry/predict/vendor-trio, drill/face/chamfer/project_curve TIDY.

**Bugs (fix first):**
- [x] **S.1 feedopt wipes every MoveIntent** (`feedopt.rs:166-188` rebuilds via intent-less
  `rapid_to`/`feed_to` which hardcode Unknown; feedopt runs as the LAST dressup) — with
  `feed_optimization` on, the final toolpath loses all Retract/EntryPlunge/Lead tags:
  air-cut % inflates, `is_steady_state_for_gate` transit filtering dies, F-039 modulator
  stops skipping plunges. Fix: `MoveType::with_feed_rate(f)` in-place mutation (the match
  already exists 3×: feed_modulation.rs:577,639, feedopt.rs:170 — dedup S.4 composes). [S]
  **FIXED 2026-07-06**: added `MoveType::with_feed_rate(self, feed_rate) -> MoveType` to
  `toolpath.rs` (preserves variant + arc i/j, no-ops on `Rapid`); migrated all 3 hand-rolled
  match sites onto it (`feed_modulation.rs` probe-feed loop + per-move apply, `feedopt.rs`
  third pass). `feedopt::optimize_feed_rates_inner` now clones `toolpath` and mutates
  `move_type` in place via `with_feed_rate` instead of re-emitting through the intent-less
  `rapid_to`/`feed_to`/`arc_*_to` builders — every move keeps target, `MoveType` variant,
  arc offsets, and `intent`; only feed changes. Added regression test
  `optimize_feed_rates_preserves_move_intents_and_geometry` (Linking rapid, EntryPlunge,
  FinishingCut linear + arc, Retract rapid via the `_with_intent` emit helpers) asserting
  every move's intent, target, `MoveType` discriminant, and arc i/j survive the pass.
  Module doc updated to describe the actual preserved-fields contract.
- [x] **S.2 vcarve/inlay/chamfer hardcode stock top at world Z=0** (`vcarve.rs:118`,
  `inlay.rs:168` emit `-depth`; `chamfer.rs:79` `cut_depth: -depth`; adapters pass no
  top_z) — the F-028 class `face.rs:44-52` documents fixing; trace + face were fixed,
  these three weren't. Latent: wrong-Z cuts on non-identity setups. [S-M]
  **FIXED 2026-07-06**: added `top_z: f64` to `VCarveParams`/`InlayParams`/`ChamferParams`
  (mirrors `TraceParams`/`FaceParams`); vcarve/inlay emit at `top_z - depth` (male +
  female + flat-clearing pocket in inlay), chamfer's `ProfileParams.cut_depth` now
  `top_z - depth`. All three adapters (`execute.rs` generate_vcarve/generate_inlay/
  generate_chamfer) pass `ctx.heights.top_z`. Existing test literals updated with
  `top_z: 0.0` (preserves old `-depth` contract at world-zero stock top); added one
  `top_z = 5.0` regression test per op asserting cuts land at `top_z - depth` and
  rapids stay at `safe_z`. Updated external callers: `tests/param_sweep.rs`,
  `tests/capability_link_moves_safety.rs`.
- [x] **S.3 engaged-diameter-at-DOC has two hand-maintained implementations**
  (`feeds/mod.rs:108-147` suggest vs `tool/vbit.rs:122`+`tapered_ball.rs:166` gate; clamps
  ALREADY differ in kind) — one edit → suggest/gate derating drift → false chipload trips.
  Minimum fix: parity sentry test across shapes × DOCs; better: one source. [M]
- [x] S.4 minor: `feed_modulation.rs:528-538` binding-tag arm order misreports ChiploadMax
  vs machine cap tie (diagnostic only, BandMid legacy path).
  **FIXED 2026-07-06**: audit confirmed — the `ChiploadMax` tie-check ran before the
  `MachineMaxFeed` check in `band_mid_feed_for_move`'s clamp-arm chain, so a machine-cap
  tie with the band ceiling reported `ChiploadMax`. Reordered so `MachineMaxFeed` is
  checked first. Added `band_mid_tie_between_machine_cap_and_ceiling_reports_machine_cap`
  pinning the tie (`max_feed_mm_min` set equal to `band_ceiling`, light engagement forces
  the clamp) to `BindingConstraint::MachineMaxFeed`.

**Cancellation (the P0.8 class, quantified for 2D):**
- [x] **S.5 Zero of 10 flat 2D ops can be cancelled** — they return `Toolpath` not
  `Result<_, Cancelled>`; shared `depth::toolpath_at_levels` takes no token. Worst loops:
  project_curve (drop-cutter per point), vcarve/inlay (scanline distance field), pocket
  (unbounded offset loop). 80% fix: one cancel-aware `toolpath_at_levels` variant checking
  between levels (covers pocket/profile/zigzag/trace/face) + in-op checks for
  project_curve/vcarve/inlay. [M]
  **FIXED 2026-07-06**: added `depth::toolpath_at_levels_with_cancel` (checks `cancel` as
  its first statement, then before each Z level) and `depth_stepped_toolpath_with_cancel`
  (both plain fns now thin `never_cancel` wrappers); every flat-2D op adapter
  (pocket/profile/zigzag/trace/face) in `compute/execute.rs` now rebuilds
  `|| ctx.cancel.load(Ordering::SeqCst)` and routes through the shared choke point.
  Pocket additionally got `pocket_contours_with_cancel`/`pocket_toolpath_with_cancel`
  polling once per offset ring (its own unbounded `loop {}`). project_curve got
  `project_curve_toolpath_with_cancel` polling every 64 resampled points in the
  per-ring drop-cutter loop. vcarve got `vcarve_toolpath_with_cancel` polling once per
  scan line; inlay's female/male halves (`female_toolpath_with_cancel`,
  `male_toolpath_with_cancel`) compose vcarve's + pocket's cancel-aware entry points and
  poll once per scan line for the male plug's own distance-field loop. Sentry
  `cancellable_families_honour_a_preset_cancel_flag` extended from 11 to 19 of 23
  families (Drill, AlignmentPinDrill, Rest, Chamfer remain uncancellable — out of this
  pass's file scope). `ExecutionContext` doc updated to list all 19.

**Dedup (ranked):**
- [x] **S.6 Canonical `MoveRemap::remap_spans`** — span-remap filter_map implemented ×4:
  dressup.rs:139, arcfit.rs:216, condition.rs:150 (byte-identical), tsp.rs:460 (variant
  w/ foreign-intrusion check). Home: toolpath_spans.rs. Drift here corrupts span→move
  mapping in three passes. [M]
- [x] **S.7 Finish 2D emitter consolidation** — pocket/profile/trace/zigzag still open-code
  rapid→plunge→feed→retract; vcarve/inlay/project_curve already use the shared emitters.
  Only real difference is climb reversal (point order). [S]
  **FIXED 2026-07-06**: migrated pocket (`contours_to_toolpath`), profile
  (`contour_to_toolpath`), trace (`trace_ring`) onto `emit_closed_contour_with_intent`,
  and zigzag (`lines_to_toolpath`) onto `emit_path_segment_with_intent` — climb/direction
  reversal stayed exactly where it was (point order before the call). Byte-identical for
  every real input; the shared closed-contour emitter's `< 3 points` no-op guard is
  stricter than the old bare `is_empty()` guard only for a theoretical 1-2 point ring,
  which none of these ops' actual contour sources (`pocket_contours`, `profile_contour`,
  closed `Polygon2` rings) ever produce — documented inline in trace.rs. No test
  MoveIntent assertions needed updating: all four ops already emitted the correct
  intents (ClearingCut for pocket/zigzag, FinishingCut for profile/trace) by hand before
  this pass, never the `Unknown` default.
- [x] **S.8 One "derate LUT chipload band" helper** — the wrapper around shared
  `doc_derating_scale` re-implemented ×4 (feeds/mod.rs:866, suggest.rs:1422 — openly says
  "Mirrors feeds::calculate", tool_load/chipload.rs:428, tool_load/mod.rs:257). Preserve:
  suggest needs both bounds, gates accept half-band. Retires the ChiploadBounds mirror
  comment for good. [S]
- [x] **S.9 `apply_boundary_clip` stock-polygon build ≡ `resolve_containment_polygon`**
  (session/compute.rs:1470-1519 ≡ :1381-1430, self-admitted "mirrors" comment at :1119)
  — clip vs generation can disagree on machinable region. [S]
- [x] S.10 Reuse `rapid_order_barriers()` (inlined ×3: condition/arcfit/dressup); + 
  AnnotatedToolpath destructure/rebuild combinator (~10 passthrough sites) [S/M]
- [ ] S.11 Two parallel invalidation mechanisms (declarative compute_stale_set vs ~30
  imperative sites in mutation.rs) — route through one incrementally [M]
- [x] S.12 Sim-request assembly duplicated (run_simulation :1673 vs
  simulate_candidate_isolated :865) [M]
- [ ] S.13 2D helpers: point-to-polygon distance (vcarve.rs:42 ≡ inlay.rs:189 →
  `polygon::min_edge_distance`); `Polygon2::xy_bounds()`; drill's two peck loops [S]

**Dead-code sweep (one small PR):**
- [x] S.14 `polygon::pocket_offsets` dead (pocket.rs reimplements inline — delete or use);
  `ChamferParams.tool_radius` dead field — **REMOVED 2026-07-06** (struct field, adapter's
  `tool_radius: ctx.tool_def.radius()` computation, and the test literals in chamfer.rs /
  param_sweep.rs / capability_link_moves_safety.rs all deleted alongside the S.2 fix);
  `DrillCycle::Dwell(f64)` payload never read;
  `project_curve.rs:243` unreachable Center arm; `GeometryClass::Shallow/SteepTerrain`
  never constructed (classify_3d_terrain always returns MixedTerrain; unreachable arm
  suggest.rs:1839); `_setup_id` on export_gcode ×2; orphaned doc blocks
  session/compute.rs:1831-1861 + :37-46; condition.rs FEED_EPS vs arcfit open-coded 1e-6;
  dressup.rs bare 0.01 tolerances ×5; compute/spans.rs is_cutting_move ≡
  MoveType::is_cutting; adaptive/mod.rs inline eprintln debug harnesses ×8;
  set_tool_param tail ≡ mutation.rs invalidate_tool; find-setup-by-toolpath open-coded ×2.
- [ ] S.15 DECISION NEEDED (user): feedopt vs feed_modulation are parallel feed-adjust
  engines — is `feed_optimization` still a supported surface, or legacy to deprecate? [L]

## Change log

- 2026-07-05: doc created from three-agent finishing review + hand verification (P0.1-P0.3
  verified in source; P0.4-P0.7 agent-reported with quoted evidence). Opus adjacent-area
  sweep launched (R1-R3).
- 2026-07-05 (later): R1-R3 landed and folded in. New verified P0s: P0.8 (7 uncancellable
  mesh-finish ops — the 2-hour-hang class; `from_mesh` sites + `execute.rs:932` checked by
  hand), P0.9 (adaptive3d stl max() collapse). R1 added the MS saddle divergence as a P2.1
  prerequisite + Polygon2 hardening list; R3 confirmed rest_grid/planner_engagement display-
  shift gap (latent, fix via core `AnnotatedToolpath::translated()`) and CLEARED the F-024
  non-identity worry (frames are paired by construction). ramp_finish stl mechanism elevated
  from P3.3 → fix alongside P0.1.
- 2026-07-05 (implementation): ALL P0s FIXED + verified (Sonnet agent waves, uncommitted).
  Wave 1: P0.1-P0.7 + P0.9 + the R3 frame fix (core `AnnotatedToolpath::translated()` with
  full destructure — R3 fix-plan items 1-4 all landed, including the M-sized core promotion).
  Wave 2: P0.8 — all 7 ops got `_with_cancel` variants (slope.rs wrapper pattern, zero test
  churn), cancel sentry now pins 11 families, ExecutionContext doc rewritten. R1.1 doc fixed
  with P0.3. P0.9 resolved as axial-only policy (planner has NO radial mechanism — see F.1).
  Wave 3 verify: core lib 1947 pass / 4 pre-existing reds (verified identical at HEAD via
  throwaway worktree; see F.3), viz 203/203, workspace clippy zero-warning after 6 mechanical
  fixes in steep_shallow. Every new regression test passes. NOT yet run: slow integration
  sentries + param sweeps (geometry-changing fixes may shift fingerprints) — run /verify
  before committing.
- 2026-07-06 overnight: P1 waves, **verified green** (core lib 1982 pass / same 4
  pre-existing reds, viz 203/203, clippy zero-warning): P1.1 `point_runs.rs` (all 7 splitter
  sites migrated; scallop+spiral chord bugs FIXED with regression tests; R1.3 raster intent
  param), P1.6 `grid2.rs` (rest_field migrated, 9 wrappers deleted; detect_rest_valleys
  phase-split deferred), P1.7 `grid_field.rs` + `marching_squares.rs` (contour_extract
  1235→600; ONE saddle convention — only boundary::model_silhouette flips, no live test
  pinned it; bool-grid center test degenerates to a permanent tie, tie-break documented),
  P1.2 `finish_setup.rs` (scallop/ramp/steep_shallow migrated; execute.rs sentinel use +
  waterline z-ladder deferred to the execute-owner pass), P1.4 (waterline cancel path
  deduped + rayon parallelism RESTORED, `emit_closed_contour_with_intent` added + waterline
  migrated; steep_shallow emitter migration deferred; dead `chain_contours` deleted).
  **INTERRUPTED by API session limit (resets 9:30pm Auckland 07-06):** P1.5 pencil split
  never started (verified: no partial edits), execute.rs-owner pass (R2.3-R2.8 + sentinel
  migration + F.3 winding fixture) not launched, P1.3 + P1.8 + fresh cleanup sweep remain.
  Tree left compiling + fully green at the documented baseline.
- 2026-07-06 (flat-2D S.5 + S.7, parallel agent wave): the 10 flat-2D ops (pocket, profile,
  zigzag, trace, face, project_curve, vcarve, inlay, +drill/rest/chamfer left out of
  scope) are now cancellable via the same `*_with_cancel` convention as the P0.8 mesh-finish
  fix — single choke point `depth::toolpath_at_levels_with_cancel` for pocket/profile/
  zigzag/trace/face, dedicated per-op polls for project_curve (per-64-points drop-cutter),
  pocket (per offset ring), and vcarve/inlay (per scan line). Cancel sentry now covers 19/23
  families (up from 11). S.7 emitter consolidation landed alongside: pocket/profile/trace
  migrated onto `emit_closed_contour_with_intent`, zigzag onto `emit_path_segment_with_intent`
  — byte-identical output, no MoveIntent changes needed (none of the four ever emitted
  `Unknown`). Not run: `/verify` (parallel-agent wave; a separate verifier pass follows).
- 2026-07-06 (campaign close — FINAL GATE GREEN): all remaining P1 + Round-2 items landed
  and verified by the comprehensive verifier. Highlights: F.3 fixture winding flipped the
  4th pre-existing red GREEN; S.3 parity sentry confirms suggest/gate engaged-diameter
  PARITY HOLDS (delegation impossible — FeedsInput carries no cutter instance); S.9's two
  boundary-resolution copies were byte-identical (no live bug), now single-sourced; cancel
  sentry 19/23 families (Drill/AlignmentPinDrill/Rest/Chamfer remain, all short-loop);
  two Opus-sweep premises refuted with evidence (pocket_offsets is a live regression
  vehicle; Dwell payload IS read by drill_metrics).
  FINAL NUMBERS: core lib 2009 pass / 3 pre-existing reds / 12 ignored; viz 203/203;
  cli 14/14; workspace clippy zero warnings.
  Incident note: one agent ran git stash/pop on the shared tree (forbidden) — forensics
  clean: no stray stash, no conflict markers, all concurrent work intact.
  REMAINING (next campaigns): P2 selective-finishing feature (+ prereqs R1.5/R1.6),
  P3 opportunistic, R2.1/R2.2, F.2, S.11, S.13, S.15 (user decision: deprecate
  feed_optimization?). NOT YET RUN: slow integration sentries + param sweeps —
  run /verify BEFORE committing this tree.
- 2026-07-06 (R1.5/R1.6/R1.7-PIP): `Polygon2` hardening for the P2 selective-finishing
  prereqs. Added `has_self_intersection`/`repaired` (geo boolean self-union resolves
  pinched/bowtied rings) and wired the repair into `offset_polygon` ahead of the existing
  `catch_unwind` backstop; added `union`/`intersection`/`difference`/`union_all` via
  `geo::BooleanOps`/`unary_union`; added `contains_point_eps` for MS-grid-aligned boundary
  points; collapsed `boundary.rs`'s test-only PIP duplicate onto `Polygon2::contains_point`
  and made the canonical `polygon::point_in_polygon` `pub(crate)`. New unit tests in
  `polygon.rs` cover bowtie repair, all three boolean ops (including hole-producing
  difference), disjoint/overlapping union, `union_all`, `contains_point_eps` boundary
  cases, and a unit-level rerun of the captured R1 cavalier-panic asset. NOT run: cargo
  (build/test/clippy) — a separate verifier pass covers this tree; see the PR/handoff notes
  for anything that needs a second look before commit.
- 2026-07-06 (wanaka_suggest_baseline re-baseline): fixed the stale-red sentry noted in
  MEMORY as needing re-baseline post-unified-load-model. Cause: the 2026-06-20 recalibration
  made deflection back-off long/thin-tool-only, so Back Rough / 3D Rough 6's stub endmill is
  chipload/power-bound and the vendor_ap envelope clamp (9.0→~5.4 mm) now wins before
  deflection ever fires. Assertions updated in `wanaka_suggest_integration.rs`; doc header
  and R4 pin-convention note rewritten to match. `cargo test -p rs_cam_core --test
  wanaka_suggest_integration` — 3 passed, 0 failed.
- 2026-07-06 (P2 selective finishing, end-to-end): P2.1 region polygons (EDT dilate +
  marching squares + containment grouping, `RestFieldResult.region_polygons`,
  `AnnotatedToolpath.rest_regions` shifted by `translated()`); R1.5/R1.6/R1.7-PIP Polygon2
  hardening (boolean ops via geo::BooleanOps, self-intersection detect+repair before
  cavalier offset, `contains_point_eps`, PIP test-helper dedup); P2.2 core+GUI+MCP
  (`DerivedRestRegions`, set-clip as the sole walk, fail-hard staleness, picker,
  boundary-edit staleness gap closed); P2.3 pre-clip in all 7 finish ops + scallop
  covered-mask/off-footprint fix + guarded helical connector (fixed a P0.4-class chord
  the sentry caught in review, plus a latent multi-region cutting bridge); rest heatmap
  overlay rendered (task #3, one source of truth with region polygons via shared
  threshold + `rest_ramp_color`). Gates: workspace `cargo check --all-targets` clean,
  clippy -D warnings clean, core lib 2059/2062 (3 pre-existing known reds only),
  viz 203/203, cli+mcp green. NOT yet run: slow integration sentries + param sweeps
  (/verify) — required before commit. Live wanaka200 validation pending.
- 2026-07-06 (P2 LIVE validation on wanaka, first pass): pencil rest_depth + reference
  picker work live (5906 moves); THREE GUI-path bugs found and sent to fix (core path
  unaffected — it fail-hards and set-clips correctly):
  (1) GUI/MCP generate with an ungenerated DerivedRestRegions source silently falls back
  to the stock rect (soft warning only, not even surfaced over MCP) — must fail-hard with
  core's wording; (2) controller keeps the derived boundary only when union_all yields
  EXACTLY ONE polygon — multi-region terrain (the common case) silently degrades to stock,
  and source regeneration does NOT mark dependents stale (cached full-part result returned);
  (3) rest heatmap overlay invisible in viewport (height-plane 0.15 hidden-mode opacity).
  Baseline numbers captured: full-part fine scallop Ø2 = 4451 moves / 7788 mm cut;
  coarse Ø6 = 4535 moves / 7757 mm; "selective" pre-fix = 4443 moves / 7919 mm (= fallback).
- 2026-07-06 (P2 live-validation fixes, GUI path): all three findings FIXED (viz 206/206):
  worker enforcement clip now calls core `apply_boundary_clip_multi` directly (duplicate
  single-polygon approximation killed); controller fail-hards with core's exact wording via
  `ComputeStatus::Error` (no stock fallback); `mark_derived_rest_dependents_stale` sweep on
  result-drain + toolpath-removal; heatmap got a dedicated uniform buffer
  (`REST_HEATMAP_OPACITY = 0.6` — root cause was the shared sim-mesh uniform stuck at the
  0.15 hidden-mode value); BONUS fix: `pending_upload` now fires on selection change within
  a setup (height planes were stale too). NEW FOLLOW-UP (latent, pre-existing):
  `notify_mcp_toolpath_complete` only fires from `drain_compute_results`, so ANY
  submit-precondition fail-hard (incl. FromRemainingStock) leaves the MCP oneshot
  unresolved — MCP callers of a failing generate get no response. Fix in next batch.
- 2026-07-07 (P2 LIVE validation COMPLETE on wanaka): full chain works end-to-end via MCP.
  Numbers (Ø2 tapered ball fine scallop, defaults): SELECTIVE (DerivedRestRegions from
  pencil rest_depth vs Ø6 ball reference) = 4605 moves / 1593 mm cutting / 7938 mm rapid;
  BASELINE all-over = 4451 moves / 7788 mm cutting / 648 mm rapid → ~80% cutting-distance
  reduction; composite screenshot confirms cuts confined to disjoint region islands.
  Heatmap overlay renders with legend (0.05→1.87 mm ramp). Fail-hard verified live with
  core wording; dependent staleness verified (source regen → selective stale, baseline
  untouched). TWO more GUI bugs found+fixed during validation: (a) `generate_via_core`
  narrowed core's AnnotatedToolpath to (toolpath, spans), dropping rest_grid/rest_regions/
  planner_engagement on the ENTIRE GUI worker path (root cause of "produced no rest
  regions" + invisible heatmap; carry the full annotated struct now); (b) MCP oneshot
  never resolved on submit-time fail-hard (all 10 early-return sites now route through
  `fail_toolpath_submit` which sets Error status AND resolves the MCP waiter; also fixed
  latent "no tool assigned leaves status stuck" bug). viz 208/208.
  Minor follow-up noted: pencil marks ITSELF stale after generating (cosmetic);
  rapid-linking between region islands is TSP-naive (7.9 m rapids — optimization target).
- 2026-07-07 (P2.5 — RegionSet consolidation + generic rest analysis): new
  `region_set.rs` replaces every hand-rolled copy of point-in-any-region /
  union-collapse / per-region keepout+offset (session/compute.rs's two, the GUI worker's
  two — fixing a latent divergence between them — and the 7 mesh-finish ops' containment
  closures); `RestAnalysisConfig` makes rest-depth analysis op-agnostic (any toolpath, its
  OWN tool as fine cutter), wired as a shared post-generation step in
  `execute_operation_annotated_with_regions` with pencil-detector precedence (skip if the
  op already attached its own rest artifacts). Full plumbing: `ToolpathConfig`/
  `ToolpathEntry` field, project-IO round-trip (core + viz, both new `#[serde(default)]`
  fields), GUI "Rest Analysis" panel section, `ComputeRequest` threading, MCP
  `set_rest_analysis_config` + `get_toolpath_params` surface. Verified: core lib 2069
  pass / 3 pre-existing reds (adaptive3d peck-plunge/rapid-segment/planner-sim-parity,
  unrelated to this work) / 12 ignored; viz lib 211/211 (one `cancelled_toolpath_returns_
  partial_debug_trace` flake reproduced in isolation as a pass — timing-sensitive,
  unrelated). rs_cam_cli / rs_cam_mcp edited mechanically (ToolpathConfig literal sites)
  but NOT compiled under this session's cargo permissions — flag for a follow-up
  `cargo check -p rs_cam_cli` / `-p rs_cam_mcp` before commit. Deferred: advisor +
  steep/shallow alternative signal (pre-existing P2.4 items, untouched); region-island
  rapid linking (pre-existing follow-up, untouched).
- 2026-07-07 (B+C live validation round 2, new binary): generic chain CONFIRMED live —
  scallop w/ rest_analysis (self/machined-stock reference) renders heatmap (0.15→3.04mm)
  + feeds DerivedRestRegions on a fine scallop directly (3895mm cutting vs 7788 baseline),
  no pencil middleman. Learning: region QUALITY is the knob — self-probe depth threshold
  0.15 yields one giant half-part region (rings fill it → only 50% saved vs pencil-derived
  islands' 80%); threshold BELOW the coarse cusp height (0.05 < 0.1) yields hundreds of
  cusp-stripe slivers and minutes-long per-region generation. NEW FOLLOW-UPS:
  - [ ] MCP cancel/timeout for in-flight generate (no way to abort over MCP; calls block)
  - [ ] sliver/region-count guard in region_polygons_from_mask (min area, merge, cap+warn)
  - [ ] per-region scallop cost warning when region set is pathological
  - [ ] region-quality advisor: threshold/detector choice per part (ties into P2.4 +
    the pencil valley-detection investigation)
- 2026-07-07 (pencil valley-targeting investigation + fix, Fable personally, UNCOMMITTED):
  full diagnosis + fix in `planning/pencil_investigation_2026-07.md`. Root causes (4):
  (1) centerlines were the medial axis of the `rest > mvd` mask — on textured relief the
  mask is the ENTIRE rough area, so the skeleton was a space-filling hairball; (2)
  `trace_skeleton` treated every 8-connected staircase corner (raw degree 3) as a junction
  → spine shredded to 0.7mm median fragments, min_cut_length then discarded 86% of length
  (coverage 0.137) — the user's "non-ideal spots" were the surviving ≥2mm shards; (3)
  offset passes width-blind (always 1+2 at fixed stepover); (4) hysteresis per-COMPONENT
  peak gate carpets when texture connects to deep trunks (dial dead on real terrain).
  FIX (rest_field.rs +~700, pencil.rs +53; Sonnet implemented to spec, Fable red-greened):
  NMS+hysteresis ridge extraction with prominence δ=max(0.1×mvd, 0.005mm), trusted-subset
  box-smooth + bilateral-trusted NMS pairs (kills trust-edge artifact ridges),
  transition-count junction tracing + ortho-preferred walk, cleanup_ridge_graph
  (spur prune + through-merge, pre-length-filter), per-BRANCH median-rest saliency gate,
  `RestCenterline{points, half_width_mm}` + width-aware offset-pass cap. Plane-wall V
  limitation documented (constant rest plateau has no ridge → Dihedral detector's turf);
  rest_field test fixtures converted plane-V → gaussian trench (numerically validated).
  VALIDATION: harness on wanaka terrain — coverage 0.137→0.80, mvd dial sweeps
  5196→2255→508 centerlines (0.15/0.5/1.0), lines follow dark creases; live GUI 464 runs
  ≈20mm avg (was 726×3mm confetti); gates: core lib 2072 pass/3 known reds, clippy clean.
  Sim collisions/air-cut on wanaka attributable to F.4 (virgin stock), baseline-diffed.
  NEW FOLLOW-UPS: F.4 FromRemainingStock regen catch-22; F.5b include_rapids intent-vs-type
  filter; F.6 export_gcode accept_unmodeled ignored. Mask→region_polygons/heatmap paths
  untouched (P2 semantics preserved).
- 2026-07-07 (F.4 fix + rest-controls UX consolidation, UNCOMMITTED, live-validated):
  F.4 = phantom prior-stock snapshot: `SimGroupEntry.phantom_prior_stock` records a
  `prior_stocks` entry for the FIRST enabled-but-ungenerated FromRemainingStock op per
  group (validity rule: everything before it must be generated → sim→regen ladder,
  one rung per sim); shared `PhantomPriorStockScan` keeps core + GUI builders identical;
  GUI submit gate consolidated onto the same `prior_stocks` map (deleted the
  boundaries()-position/checkpoint re-derivation). LIVE: fresh wanaka load → the four
  stuck ops (Rivers/Lakes/3D Rough 6/3D Finish 6) regenerated rung-by-rung for the
  first time since the fail-hard fix; disabled ops correctly don't block the ladder.
  UX = pencil Geometry gets ONE "Rest reference" group (Machined stock ⇔
  FromRemainingStock / Reference tool ⇔ Fresh + picker), generic "Use remaining stock"
  checkbox hidden on pencil (it silently overrode the picker); Rest Analysis section
  hidden on rest_depth pencils ("produced by the detector"), demand-driven elsewhere:
  `session::auto_enable_rest_analysis_for_source` fires from set_boundary_config (MCP)
  and the GUI sync path when a DerivedRestRegions consumer appears ("Producing rest
  regions for: ..."), manual toggle relabeled "Compute rest heatmap".
  BONUS R2 VALIDATION (first honest one, unblocked by F.4): pencil vs real machined
  stock after the full chain = rest_reference_mode 2, 669mm cutting vs 9265mm with the
  Ø6-ball analytic reference (~14× overestimate exposed — the finish used the same Ø2
  tool, so true rest = boundary bands/edges only); all 30 chains centerline-only
  (width-aware pass cap working); pencil adds 0 rapid collisions (was 14 on virgin
  stock). Gates: core 2078 pass/3 known reds, viz 212/212, clippy clean.
- 2026-07-07 (tech-debt pair, COMMITTED d0d6d75 + 482be35, live-validated):
  sliver-region guard (MAX_REST_REGIONS=64 cap + warn in region_polygons_from_mask;
  classify_rest_regions pathology → GUI captions in Rest Analysis + Boundary picker) and
  MCP cancel_generation + timeout_s on generate_toolpath/generate_all (toolpath-lane-only
  cancel event; late-oneshot-after-timeout safe by fresh-channel construction).
  Live: timeout_s=2 → still-running; mid-flight cancel named "Back Rough", reverted to
  Pending, siblings untouched, idle no-op, post-cancel regen clean. Closes the
  "MCP cancel/timeout" and "sliver/region-count guard" follow-ups; the region-quality
  advisor (P2.4) is PARTIALLY covered by classify_rest_regions (threshold-below-cusp +
  giant-region advice); full advisor remains open.
- 2026-07-07 (unified-finishing P0 probe, UNCOMMITTED as of writing, live-run):
  `CycleTimeBreakdown` per-MoveIntent time on the F-034 integrator (per-toolpath +
  project `runtime_by_intent` on the cut trace, `get_cut_trace.toolpath_summaries`
  block); found+fixed F-036b1 desync (post-modulation re-walk rewrote total_runtime_s
  but not the breakdown). PROBE VERDICT (full numbers in
  planning/unified_finishing_pass_plan.md P0 results log): strict finishing overhead
  25.2%, detail+finishing 39.3% → GATE PASSES, proceed P1+P2. Headlines: pencil 97.3%
  overhead (335 s plunge vs 14 s cut), Rivers 95.3% (1461 s EntryPlunge), Finish 6
  10.1× naive (junction/accel physics on 0.31 mm segments — P3 justified on time);
  Finish 6 emits 656 s of untagged (Unknown-intent) link feeds — tagging gap for P1.
  Gates: clippy clean, core lib 2084/3 known reds, f034 10/10 + f036b 5/5 sentries,
  viz 227/227.
- 2026-07-07/08 overnight (P1 quantitative linker + stock-aware entry descents):
  W1 `retract_link_time`/`surface_link_time` (integrator-costed link candidates) +
  `LinkKinematics` bundle on ExecutionContext; W3 boundary-clip MoveIntent tagging
  (kills Finish 6's 656 s unknown_s); W4a pencil hookup = cost decision (cap stays
  the candidate filter); W2 REWORKED after live A/B caught 151 rapid collisions —
  mesh-derived descend heights are unsafe on FromRemainingStock (river channels
  hold stock far above the mesh); replaced by `dressup::optimize_entry_descents`
  post-pass splitting EntryPlunge feeds at `TriDexelStock::max_top_z_in_disc(x,y,
  tool_r) + PLUNGE_CLEARANCE_MM` (prior dexel for rest ops, stock-top plane for
  fresh; provenance-remapped spans). HEADLESS A/B (new harness
  tests/p1_headless_ab_wanaka.rs, GUI-modulation-equivalent): project −16.6%
  (10692→8920 s), Rivers −54%, Lakes −35%, Finish 6 −9.1%, roughing controls
  unchanged, 0 rapid collisions, unknown_s = 0 everywhere. Open: ComputeRequest
  machine threading (GUI pencil cost decision), W4b scallop links, live-GUI
  collision confirm.
- 2026-07-08 (P2.b decomposition + conditioning, UNCOMMITTED as of writing):
  new `finish_planner` core module — `decompose(slope_map, covered, creases,
  tool_radius, params)` → three conditioned bands (Shallow raster / MidSteep
  scallop / VerySteep waterline) as clean polygons via the shared
  `region_polygons_from_mask`, plus the crease corridor rule
  (`half_width_mm < corridor_k × tool_radius` stays in-band; wider = own
  region, corridor carved out of band masks). R1 conditioning = hysteresis
  (multi-source seeded flood, enter/leave thresholds), EDT-based
  morphological close, min-area absorption (deterministic order + majority
  vote, ≤4 passes). `FinishPlannerParams::for_tool` carries the new dials
  (45/65 thresholds, hysteresis 10°, corridor K=2, close r/2, min-area
  4×(2r)²); `planned_regions_to_svg` is the P2.b debug/visual surface.
  R1 ACCEPTANCE PASSED: wanaka → 1 region (3 raw steep islands = diagonal
  river-channel walls, correctly absorbed — crease territory), O(10) ≫
  satisfied; SVGs in target/finish_planner_debug/. FELL-OUT SHARED FIX:
  `polygon::detect_containment` lacked even-odd nesting — island-in-hole
  (dome cap inside a band annulus) was silently consumed as a hole of the
  outermost polygon; now containment-depth based (even = top-level, odd =
  hole of innermost even-depth container), depth ≤ 1 callers byte-identical.
  Gates: clippy workspace clean; core lib 2116/3 known reds (11 new
  finish_planner + 2 new polygon tests); cli 9/9; viz 227/227; wanaka
  acceptance 1/1. Next: P2.c per-band generation + naive concat.
- 2026-07-08 (P2.b follow-up, user-caught): classification surface was WRONG —
  the drop-cutter heightmap is the ball-CENTER offset surface and hides
  steepness at feature scales ≤ ball radius (wanaka true surface 38.5% ≥45°,
  offset surface 0.1%, max 89° vs 52°). Added
  `finish_setup::build_classification_surface_with_cancel` (tiny bare-surface
  probe, rest_field precedent; grid cell-compatible with the generation
  surface) + stencil-safe 1-cell coverage erosion in decompose (min_z-clamp
  boundary cliffs). True-surface wanaka: 54+2 raw islands → 3 regions (mid-
  steep tracks the real mountain range). Design decision #3 amended:
  CLASSIFY on true surface, GENERATE on offset surface — decomposition is
  tool-independent (multi-tool cascade ready). steep_shallow op shares the
  offset-surface blind spot (quantified via
  `wanaka_slope_distribution_diagnostic`; left as-is, planner supersedes).
  Gates re-green (clippy clean, finish_planner 11/11, finish_setup 10/10).
- 2026-07-08 (scallop ring-cascade exponential — found by the P2.c A/B,
  UNCOMMITTED): region-scoped scallop on wanaka's dendritic mid-steep band
  hung for hours at fine scallop heights. Root cause: `offset_polygon`
  ADDS arc-approximation vertices at concave corners on every call and
  never removes any, so the iterated inward-offset ring cascade compounds
  ~15–25% vertices/ring — measured 1178 → 261 000 vertices by ring 25
  (10 s/offset and doubling). Convex boundaries (classic full-footprint
  scallop) gain only ~4 points/ring, which is why 43 unit tests + selective
  scallop at normal heights never saw it; REACHABLE FROM THE GUI (heights
  down to 0.01 on any dendritic region). Fix: `decimate_ring_polygon` in
  the ring loop — drop-only decimation at 0.75×heightmap cell spacing
  (never adds points; already-at-density rings pass byte-identical; sliver
  fragments <3 points culled). Cost curve h=0.1/0.05/0.02/0.011:
  pre-fix 2.6 s / 4.3 s / >30 min (killed) / unmeasured → post-fix
  2.1/2.4/2.9/3.6 s. Scallop lib tests 43/43. FOLLOW-UP (root of the root):
  fix the inflation inside `offset_polygon` itself with a sweep-validated
  pass — benefits pocket/adaptive/rest cascades too; tracked, not a P2.c
  rider (blast radius = every offset consumer's geometry).
- 2026-07-08 (P2.c CHECKPOINT #1 VERDICT, UNCOMMITTED as of writing):
  branch B (UnifiedFinish at parity dials: raster 0.3 = A, scallop_height
  0.011 = A's effective mid-steep cusp, z_step 0.3) vs pinned branch A
  (8919.5 s project / 6883.4 s finish — reproduced the P1 headless
  baseline to 0.5 s): finish 7011.4 s (+1.9%), project 9047.5 s (+1.4%),
  collisions 0 (gate ≤4 PASSED). Intent split: cutting 5839.5 → 5137.9 s
  (−12%, the banding win — scallop rings beat 0.3 mm raster on the
  mid-steep band at BETTER held cusp) vs entry+rapid 1044 → 1874 s
  (+830 s — the naive band split makes the shallow raster plunge back in
  at every band crossing). VERDICT: checkpoint passes (no collision
  regression, tiny time cost buys strictly better steep quality, cutting
  win proven); the +830 s overhead is quantified and is exactly P2.d's
  router/surface-link target — far richer than R3's 5–8% estimate.
  Unchanged ops reproduced A bit-for-bit (Back Rough 674.8 = 674.8).
  NOTE: full B chain = 51 s wall post-scallop-fix; the first A/B's 2 h+
  was the scallop exponential + probable sysml-job contention. Harness
  keeps pinned-A B-only mode + three pathology probes.
- 2026-07-08 (COASTLINE STENCIL + P2.d ROUTER, uncommitted as of writing):
  two landings, verdicts vs the same pinned A (8919.5 s / 6883.4 s).
  (1) Classification stencil: `SlopeMap::from_z_grid_max_gradient` — per
  axis the one-sided difference with the larger magnitude, wired ONLY into
  `build_classification_surface_with_cancel` (generation surfaces keep
  central differences). Root cause of the user-observed missing coastline
  (a single-cell ~90° step reads `atan(h/(2·cell))` ≈ 34° under central
  differences, and 0° at the step BOTTOM); the correction is global on
  textured relief — wanaka steep fraction now 42.8% of covered cells vs
  the true-surface mesh statistic 45.8% ≥35° (central diff was
  under-reading everywhere). Decomposition: 3 → 4 regions (VerySteep
  442 mm² now survives min-area; coast ring registers as a continuous
  VerySteep ribbon in the raw masks; SE stretch absorbed into MidSteep —
  thin-ring absorption stays a P2.e sweep datapoint). A/B: flipped the
  checkpoint to B −4.1% project (−366.7 s), finish −5.3%, collisions 0 —
  the fixed classification routes more range to scallop-at-held-cusp
  instead of 0.3 mm raster. 3 new slope.rs unit tests incl. the
  trench-bottom case.
  (2) P2.d router (design step 4+5): per-REGION generation + greedy
  ordering by `min(retract_link_time, surface_link_time)` (F-034
  integrator), winning link EMITTED (surface link strips follower
  preamble + leader trailing retracts; boundary-checked, gouge-checked
  via the shared `surface_link` module), steep-first seeded,
  `LinkKinematics` via the same ctx plumbing as pencil's P1 W4a. A/B:
  another −127.9 s (rapid 1397 → 1291 s within the finish op) →
  **combined B 6388.8 s finish (−7.2%) / 8424.9 s project (−5.5%,
  −494.6 s) / collisions 0**. 2-opt not built (greedy at O(4) regions —
  measure-first). Remaining finish entry_s (803 s) is internal strategy
  plunges → P3 territory. Lib battery 2126 green / 3 known reds.
- 2026-07-08 (P2.e SWEEP + DEFAULT LOCK, uncommitted as of writing): both
  sweep tiers run. Tier-1 (16 conditioning rows, decompose-only, 0.8 s):
  defaults in a stable basin, no cliffs; hysteresis=0 re-creates the R1
  island storm (115 raw islands) — hysteresis is load-bearing; coastline
  survival levers = hysteresis ↑ / min_area ↓ / close ↑. Tier-2 (8
  threshold rows through the full chain, ~9 min, all 0 collisions): steep
  monotonic (35 → −14% finish, quality-trading vs A in the 35–45° band →
  45 kept as the quality-neutral anchor); waterline is the big lever —
  55 → +27.0% finish (Z-contouring is the most expensive strategy per
  area), 75 → −16.2%, 85 flat. **LOCKED: waterline default 65→75**
  (`FinishPlannerParams::for_tool` + `UnifiedFinishConfig::default` +
  harness config). Fresh branch-B at locked defaults reproduces the sweep
  row bit-for-bit: finish 5766.5 s (−16.2%), project 7802.6 s (−12.5%,
  −1116.9 s vs pinned A), collisions 0; finish entry_s 803→489 s. Honest
  caveat: chain-scored on wanaka only — a wall-heavy fixture would
  exercise the shrunk 75–90° waterline band; revisit then. Cumulative
  unified-finish arc vs A: P2.c +1.4% → coastline fix −4.1% → router
  −5.5% → P2.e lock **−12.5% project**.
- 2026-07-08 (MATERIAL + ONE-AT-A-TIME MEASUREMENT, uncommitted): user
  asked (a) is unified better than chaining the strategies one at a time,
  (b) was material removal measured. Harness extended: removed-volume
  column (dexel `total_removed_volume_est_mm3`, was measured but never
  reported) + final stock-vs-model deviation stats (`sim.deviations`:
  positive=leftover, negative=overcut) on every chain; new branches
  `p2e_branch_a_remeasure` (pin drift 0.0 s — pin confirmed) and
  `p2e_separate_ops_branch_c` = the SAME strategies as standalone
  slope-windowed ops (Scallop ≥45° + A's raster <45°, all else identical).
  RESULTS: A finish 6883.4 s / removed 9988 mm³ / leftover mean 0.270 mm;
  B (unified, locked) 5766.5 s (−16.2%) / removed 9720 mm³ (−2.7%, same
  material) / leftover mean 0.314 mm (comparable; stats dominated by
  structurally-uncut skirts in all branches); C **9913.2 s finish (+44.0%
  vs A) / leftover mean 0.911 mm (3.4× worse)** — the offset-surface
  slope windows are the killer exactly as predicted: C1 scallop found
  almost nothing to cut (33 s cutting, 347 mm³) because the offset
  surface reads wanaka as 99.9% <45°, AND the <45° window excluded the
  same steep cells from C2's raster, so steep terrain got NEITHER
  strategy; C2's window fragmentation also ballooned rapids to 3549 s.
  All branches 0 collisions. FOLLOW-UP FLAGGED: C's finish removed
  34 208 mm³ (3.4× A) while ALSO leaving 3.4× more material + more
  overcut verts (23 880 vs 16 312) — signature suggests the standalone
  slope-windowed raster path may be missing the rim-contact trench guard
  (or similar); worth a targeted look at
  `raster_toolpath_from_grid_with_slope_filter` emission vs
  `generate_drop_cutter`'s guard. Not chased (C loses decisively either
  way), logged as a potential pre-existing standalone-op defect.
- 2026-07-08 (LIVE GUI VALIDATION of UnifiedFinish — the P2.c tail — run
  on live wanaka via MCP, op added live at parity dials + Finish 6's
  heights/dressups): coverage CONFIRMED (final stock visually identical
  to A's; classification/banding/generation all work through the GUI
  worker; 85 487 moves, 37.4 km cutting). But live does NOT reproduce
  the headless verdict — finish op reads ~9 516 s live vs 5 766 s
  headless, and the gap is fully quantified: **entry_s 3 126 s live vs
  489 s headless** — `optimize_entry_descents` runs in the GUI worker
  (execute/mod.rs mirrors session wiring) but is NOT splitting this op's
  plunges on live wanaka, while Rivers (Setup 1, face Bottom) descends
  identically live (453 s) and headless (462 s). Pattern = works in the
  zero-rooted Setup-1 frame, fails in Setup 2 (identity setup,
  world-frame stock per F-024) → suspected frame mismatch between
  `req.prior_stock` (world) and the worker's setup-local toolpath in the
  descent pass — F-024's sibling, this time in the worker path. Also:
  60 rapid collisions (baseline 4), all in the unified op, spread across
  the whole move range — likely same frame family; and
  `retract_strategy` is a NO-OP on unified output (byte-identical
  toolpaths full vs minimum) — should apply or be hidden for the op.
  (One self-inflicted detour for the record: pinning absolute heights in
  the wrong frame via MCP put the retract plane inside the stock →
  12 267 collisions; `set_toolpath_heights` frame semantics are easy to
  misuse — UX footnote.) NEXT SESSION: worker-path descent/collision
  frame audit for identity setups; acceptance = live unified entry_s
  drops to ~500 s and collisions return to baseline, closing live-vs-
  headless parity. This is the same lesson as the P2 selective-finishing
  validation: the GUI worker path catches what headless can't.
- 2026-07-08 (LIVE VALIDATION addendum — USER-CAUGHT UNCUT BAND): at
  sim END, a broad diagonal band of the hilly area shows coarse stepped
  bars where detailed hills should be — the unified op left that swath
  UNCUT (the bars read as 3D Rough 6's z-level terraces still standing,
  i.e. mountains buried in rough stock, "smooshed"). North of the band
  the finish detail is crisp. Evidence this cluster is LIVE/WORKER-PATH
  ONLY: the headless B deviation stats show NO missing band (leftover
  verts 34 381 vs A's 34 614, mean 0.314 vs 0.270 mm — a whole uncut
  band would add tens of thousands of leftover verts), while live also
  shows entry_s 6× headless and 60 collisions vs 0. Additional USER
  observations logged the same session: (1) region-clipped raster rows
  emit retract+plunge PER ROW (A's unclipped raster serpentines at the
  surface) — real emission-quality gap, part of the 4× rapid distance;
  (2) no pencil/crease moves — BY DESIGN at this checkpoint (creases
  deliberately empty; integration is the tracked tail); (3) live
  decomposition at the tapered ball's r=1 runs ~60 regions (min_area
  16 mm² — "Selected: Region 59" in the inspector), much weaker
  conditioning than the Ø6 acceptance picture; region-count-vs-tool-
  radius deserves a planner clamp datapoint. NEXT-SESSION P2.f BLOCK
  (live parity + emission quality, BEFORE P3): (a) worker-path frame
  audit (descents + collisions + THE UNCUT BAND — likely one family);
  (b) headless-vs-live stock render harness (add a stock PNG dump to
  the B-only harness so this class is visible headlessly); (c) raster
  row serpentine within regions; (d) crease integration; (e)
  retract_strategy no-op cleanup; (f) min_area floor independent of
  tool radius (60 regions at r=1 is conditioning failure territory).
- 2026-07-08 (ROOT CAUSE of the smooshed band — user-driven diagnosis):
  NOT uncut stock (scrubbed to op start: rough blobs, no bars — the
  unified op CUTS the bars) and NOT live-only (core generation). The
  mid-steep scallop band generates on an INTERNAL HEIGHTMAP at
  `cell = radius/4` where `TaperedBallEndmill::radius()` returns the
  SHANK radius (3 mm) → 0.75 mm cells; ring points INTERPOLATE that
  grid, so terrain texture finer than the cell (wanaka: 0.3–1 mm) is
  blurred out of the generation surface and the rings behead every
  knob the grid can't see — the visible bars. Waterline has the same
  disease via `sampling = 0.5 mm` marching squares. Branch A never
  suffers it: its raster emits EXACT per-point drop-cutter samples at
  0.3 mm. This is a PRE-EXISTING standalone scallop/waterline fidelity
  limitation (same r/4 heightmap in the standalone ops) that the
  unified op exposed by assigning those strategies to textured terrain
  A covered with exact-sampled raster. It is why headless B's leftover
  mean read +16% vs A — the checkpoint/sweep verdicts (time, collisions,
  blunt deviation mean) never checked per-band surface fidelity; the
  P2.e "cutting −12% at better held cusp" claim holds for the cusp math
  and FAILS on sub-cell texture. FIX CANDIDATES (P2.f top item, now
  above the frame audit): (1) re-sample emitted ring/contour points
  with exact drop-cutter Z (placement on the coarse map, Z exact —
  raster-equivalent fidelity, ~1 query/output point); (2) cell from
  TIP radius for tapered tools (`max(tip_radius/4, tolerance)`);
  (3) clamp waterline sampling to the classification cell or fall back
  to scallop on textured VerySteep. ACCEPTANCE: per-band leftover
  histogram vs A on the textured flank (add to the harness — the blunt
  mean hid this), plus the user's eyeball on the live stock.
- 2026-07-08/09 overnight (P2.f Task 1 EXECUTED — instrument + chord
  fix, headless-validated): reading the code corrected the root-cause
  DETAIL — scallop ring Z was never interpolated (`ring_to_3d` exact
  drop-cutters every vertex); the beheading was the straight feed
  CHORDS between exact points. Ring vertex spacing tracks the
  generation grid (`decimate_ring_polygon` floors it at cell×0.75 =
  0.56 mm; boundary sampling ≈ flat stepover 0.51 mm), so any knob
  narrower than a chord was decapitated by the segment crossing it —
  and valleys under chords read as leftover. Waterline shares the
  disease via 0.5 mm fiber spacing (a knob between fibers is invisible
  — no XY detour is generated; chord refinement can't fix that one,
  only finer sampling can). NOTE: at the LOCKED 45/75 dials wanaka's
  very-steep band is EMPTY (band map: 0 cells) — mid-steep scallop
  owns all textured terrain, so the scallop chord fix is the whole
  wanaka fix; waterline sampling stays a ledgered follow-up.
  - INSTRUMENT (built first, per the lesson): `p2f_fidelity_branch_a/b`
    in `tests/p2c_headless_ab_wanaka.rs` — per-band deviation histogram
    (13 signed bins; bands rasterized from the planner's own conditioned
    regions), 0.25 mm measurement re-sim (the 0.5 mm timing sim aliases
    the texture; timing still reported from standard options), top-down
    deviation PNG + raw f32 grid dump for cross-run diff maps, 6-view
    composite. Artifacts in `target/p2f_fidelity/`.
  - PRE-FIX BASELINE (hi-res): B's mid-steep overcut bins +25–45% vs A
    (−0.2..−0.1 bin: 2030 vs 1403), shallow overcut ≈2× A (scallop's
    2 mm overlap spills into raster territory), leftover ≥0.3 mm
    +48–74%. Diff map (B−A) lights up exactly the diagonal mountain
    band the user photographed. mean|B−A| = 0.054 mm.
  - FIX (`scallop.rs`): adaptive chord refinement in the ring lift —
    each kept→kept chord (including the closing wrap) is probed against
    exact drop-cutter Z every `max(cell/2, 0.15 mm)`; while the worst
    probe error exceeds the op's path tolerance the chord splits at the
    worst-error point (depth-capped 5, floor 0.15 mm = half the raster
    reference pitch so refinement can never re-create the sub-0.1 mm
    segment-junction blowup from the P0 probe). Coverage gaps under a
    chord insert an excluded point so the run-splitter retracts around
    holes instead of feeding across (bonus fix). Flat/smooth chords
    within tolerance gain ZERO points (unit-pinned). Unit tests:
    `chord_refinement_lifts_path_over_sharp_ridge` (tent-ridge mesh —
    the minimal smooshed-mountain reproducer), `chord_refinement_no_op_
    on_flat`.
  - POST-FIX B (hi-res): mid-steep overcut bins at A PARITY
    (1212/896/1403 vs A 1200/889/1403; pre-fix 1502/1267/2030);
    shallow overcut back to parity; leftover >+0.5 5515→3902 (A 3737).
    mean|B−A| 0.054→0.012 mm (4.6×); cells B gouges >0.1 mm deeper
    than A: 1955→216. Removed volume 9871 vs A 9988 (pre-fix 9720).
    HONEST TIME RE-MEASURE: finish −16.2%→−13.1%, project
    −12.5%→−10.1% (−899.7 s), collisions 0 — the 3-point give-back is
    the tool genuinely following the knobs it used to slice off.
- 2026-07-09 overnight (P2.f Task 2 forensics — live G-code REFUTES the
  descent-pass theory): exported the live project's per-setup G-code
  (GUI still open, read-only) and the unified op's entries show the
  split signature everywhere — `G0` rapid descents to exactly
  plunge-target +1 mm before every F-tagged feed. `optimize_entry_
  descents` WORKS in the GUI worker; the handoff's frame-mismatch
  hypothesis is wrong for descents (F-028: identity setups skip the
  transform precisely so emission and sim agree). What actually
  dominates live entry_s: ~20 mm DIAGONAL EntryPlunge feed legs at F75
  (V-shaped out-and-back pairs descending ~1 mm per leg) — the same
  "entry moves cutting through stock" the user flagged as PRE-EXISTING.
  Finish 6's dressups: entry_style=none (NOT a ramp dressup),
  arc_fitting=true, optimize_rapid_order=true, retract_strategy=
  minimum, link_moves=false; the F75 is feed modulation halving the
  150 plunge feed.
  **ROOT CAUSE FOUND + FIXED same night**: the diagonal legs are
  `emit_ramp` — the geometry matches exactly (rapid to plunge-target
  +2.0 mm = `ENTRY_CLEARANCE`, then out-and-back legs at 2.86° ≤ the
  3.0° ramp_angle). The live op 8 was added fresh via MCP
  `add_toolpath`, which builds dressups from `DressupConfig::for_op` →
  `for_role(UiProcessRole::Finish)` → **`entry_style: Ramp` (Roadmap
  B.5 role default)** — and `REG_UNIFIED_FINISH` carried
  `ANY_DRESSUP`, so nothing stripped it. The original Finish 6 never
  showed it because its SAVED config has entry_style=none; the
  headless harness inherits Finish 6's dressups via
  `set_toolpath_operation`, which is why headless entry_s stayed 489 s
  while the live op paid 3126 s of 3° trenches (and `emit_ramp`'s
  target-relative rapid floor rapids BELOW terrain knobs — the prime
  suspect for the +60 rapid collisions). This is DropCutter's
  documented "diagonal trench" failure mode; DropCutter/ProjectCurve
  already strip_all — UnifiedFinish had simply slipped through with
  ANY_DRESSUP at registration. FIX: `REG_UNIFIED_FINISH.dressup_policy
  = strip_all(...)` (catalog.rs), pin tests updated
  (`dressup_policy_table_is_pinned`, `normalize_for_op_applies_
  registry_policy`). The UI greys the controls from the same registry
  field. REMAINING live checks (morning, GUI restart on fixed binary):
  re-add the unified op (strip-all now applies), confirm entry_s ≈
  headless and collisions back to baseline 4, retract_strategy no-op
  question, and the user's eyeball on the chord-refined stock.
- 2026-07-09 overnight (P2.f Task 3a — raster serpentine, headless
  validated): `raster_toolpath_from_grid`'s segmented branch (min_z /
  boundary-region clipped) paid a full retract → rapid → replunge cycle
  at EVERY row; the unified op's region-clipped shallow raster was the
  main payer (489 s of entry plunges vs branch A's 57 s). The segment
  retract is now DEFERRED: when the next segment starts within one
  grid-cell diagonal (×1.05) of where the tool sits, it stays down and
  feeds there (the same surface chord an unclipped zigzag cuts at a
  turnaround). The one-diagonal threshold doubles as the gap guard — a
  single excluded/clamped point already puts segments two steps apart,
  so links can never bridge a min_z hole or leave the regions by more
  than sub-cell slack (unit-pinned: `raster_serpentine_never_bridges_
  gaps`; the covering-region parity test now asserts one entry cycle +
  two rapids for a fully-connected grid). RESULT (chord fix +
  serpentine, quality histograms unchanged from the chord-fix run):
  **B finish 5486.3 s (−20.3%), project −15.7%, collisions 0; finish
  entry 489→150 s, rapid 1105→941 s.**
  BRANCH-A INVARIANCE (verified, not assumed): A's chain totals are
  byte-identical post-serpentine, so the pinned constants stay valid.
  Census probe (`p2f_a_move_census`): A's FINAL finish toolpath still
  carries exactly one entry plunge per raster row (313) — A's
  model-silhouette boundary is applied as a POST-clip that cuts the
  path at every silhouette crossing and re-emits per-row entry cycles,
  so generation-level links at the bbox-edge turnarounds (outside the
  silhouette) cannot survive it. The unified op's links live INSIDE its
  regions ⊂ silhouette and pass through the same clip untouched.
  Serpentining A itself would need clip-aware linking — ledgered as a
  Task 3 tail, deliberately NOT done while pinned-A comparability
  anchors the campaign.
