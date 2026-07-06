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

- [ ] **P2.1 Region polygons from the rest field**
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
- [ ] **P2.2 New derived boundary source**
  - `BoundarySource` (`compute/config.rs:243`) has only Stock / ModelSilhouette /
    Geometry{imported} / FaceSelection — no computed variant.
  - Add e.g. `DerivedRestRegions { source_toolpath_id }` (or a session store for derived
    polygon sets); resolve in `apply_boundary_clip` (`session/compute.rs:1443`).
    Precondition/staleness handling mirrors the FromRemainingStock fail-hard pattern
    (`session/compute.rs:1158-1186`).
- [ ] **P2.3 Thread `ctx.boundary` into the finish family (pre-clip, not just post-clip)**
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
- [ ] **P2.4 GUI/MCP surface + advisor**
  - Overlay: rest heatmap (Layer 3 WIP) draws the SAME regions the boundary will use —
    keep one source of truth.
  - Advisor (later): suggest "scallop clipped to regions A..N with tool T" from the rest
    report — candidates ranked by `peak_rest_mm` significance.
  - Alternative detail signal for parts without a rest reference:
    `classify_steep_shallow` (`slope.rs:405`) + dilate + marching squares = steep/shallow
    region polygons; `SlopeMap::curvature_at_world` as a "high-detail" detector.

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
- [ ] **R1.5 No self-intersection/pinch repair** — offset robustness is a
  `catch_unwind` swallowing cavalier panics (`polygon.rs:169-183`); MS saddle output is
  exactly the pinched-ring shape that triggers it. Guard/repair before offset [M].
- [ ] **R1.6 No boolean ops** (union/intersect/difference) — needed to merge/subtract derived
  region polygons; wrap `geo::BooleanOps` [M].
- R1.7 Smaller: `detect_containment` nests one level only (`polygon.rs:363`); offset hole
  re-assignment heuristic can misassign on region splits (`:257-273`); `contains_point` has
  no boundary epsilon (`:418-436` — MS output sits exactly on grid-aligned edges); THREE
  ray-cast PIP copies (`polygon.rs:418`, `boundary.rs:436` test-only, `Polygon2::contains_point`)
  → collapse to one [S].
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
