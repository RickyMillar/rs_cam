# rs_cam capabilities inventory — Phase F0 primitives

> Produced 2026-08-29 by a very-thorough repo sweep for the conformal
> finishing feasibility programme (`PROGRAMME.md` Phase F0, third bullet).
> Every claim carries a file:line reference; verdicts are against the tree as
> of commit `3549f186`.

Workspace: 4 members (`rs_cam_core`, `rs_cam_cli`, `rs_cam_viz`, `rs_cam_mcp`;
`tests/step_validation` and `reference/shapeoko_feeds_and_speeds` are
excluded/standalone). Essentially all geometry lives in `rs_cam_core`.

---

## 1. Triangular mesh representation + sampling — PARTIAL (walking machinery MISSING)

**Storage — EXISTS.** `crates/rs_cam_core/src/mesh.rs:32`
```rust
pub struct TriangleMesh {
    pub vertices: Vec<P3>,
    pub triangles: Vec<[u32; 3]>,
    pub faces: Vec<Triangle>,      // parallel to `triangles`, precomputed
    pub bbox: BoundingBox3,
}
```
Constructors: `from_stl_scaled` (`mesh.rs:44`), `from_stl_bytes` (`:140`), `from_raw` (`:418`). Winding audit/repair: `check_winding` (`:232`), `fix_winding` (`:289`). Test meshes: `make_test_hemisphere` (`:880`), `make_test_flat` (`:927`).

**Per-triangle normal — EXISTS.** `crates/rs_cam_core/src/geo.rs:152-174` — `Triangle { v: [P3;3], normal: V3 }`, normal precomputed and normalised at construction. Also `Triangle::contains_point_xy` (`geo.rs:182`), `Triangle::z_at_xy` (`geo.rs:240`, planar interpolation), `Triangle::ray_intersect` (`geo.rs:205`).

**Per-vertex normals / curvature tensor — EXISTS but only inside `crest_lines`.** `crates/rs_cam_core/src/crest_lines.rs` implements Rusinkiewicz 2004 per-vertex second fundamental form with mixed-Voronoi area weighting, principal curvatures κ1/κ2 and directions t1/t2, plus the derivative-of-curvature tensor. It is a private `struct Curvature` (`crest_lines.rs:93`); the only public entry is `detect_valley_lines(mesh, &CrestParams) -> Vec<Vec<P3>>` (`crest_lines.rs:757`). **There is no public per-vertex normal / curvature API** — a new arm would have to lift it out or duplicate it.

**Spatial query — EXISTS.** `SpatialIndex` (`mesh.rs:481`), a uniform XY bucket grid: `build_auto` (`:499`), `build` (`:524`), `query(cx, cy, radius)` (`:636`), `query_into` (`:740`), `query_rect_into` (`:780`), `cell_triangles_at` (`:840`), with reusable `QueryScratch` (`:860`). `ray_pick_triangle` (`mesh.rs:443`) is brute-force after a bbox reject — documented as "single-ray picking (not per-pixel rendering)".

**Barycentric interpolation — NOT a reusable primitive.** Only two ad-hoc sites: `crates/rs_cam_core/src/fingerprint.rs:1251-1278` (rasteriser colour interpolation) and the tolerance comment at `crates/rs_cam_core/src/pushcutter.rs:38` describing `Triangle::contains_point_xy`'s `-1e-8` barycentric acceptance. There is **no** `barycentric(tri, p)` helper and no "sample field at a surface point" function.

**Half-edge / triangle adjacency — MISSING as a general structure.** The nearest thing is `crates/rs_cam_core/src/pencil_dihedral.rs`: `EdgeKey` (`:35`), `build_edge_adjacency(mesh) -> HashMap<EdgeKey, …>` (edge→incident-face map), `compute_shared_edges`, `chain_concave_edges` (re-exported through `crates/rs_cam_core/src/pencil.rs:32`). That gives edge→face incidence, **not** a half-edge structure: no opposite-half-edge pointer, no per-triangle neighbour array, no next/prev traversal, no one-ring vertex iteration. Tracing a curve across triangles (walk into a triangle, exit through an edge, continue in the neighbour) would need new code on top of `build_edge_adjacency`.

`crates/rs_cam_core/src/enriched_mesh.rs` does have `adjacent_faces` on `FaceGroup` (`:69`) and `BrepEdge` (`:85`), built by `build_enriched_mesh` (`:369`) from `step_input.rs:142-181`. **This is STEP face-group adjacency, not triangle adjacency, and STL-imported meshes never get it.** It does not satisfy this primitive.

**Practical mesh sampling is 2.5D grid-based, not on-surface.** `SurfaceHeightmap` (`crates/rs_cam_core/src/slope.rs:87`) — `from_mesh_with_cancel` (`:141`), `z_at_world` (`:312`), `covered_flags` (`:369`), `slope_map` (`:432`); `SlopeMap` (`:462`) with `normal_at` (`:741`), `curvature_at` (`:748`), `angle_at_world` (`:754`), `world_to_cell` (`:766`). This is a regular XY lattice of drop-cutter results, not a mesh parameter domain.

---

## 2. Cutter-centre / offset surface for ball-end tools — drop-cutter projection ONLY

**No inflated/offset mesh exists.** There is no Minkowski-sum surface, no offset-mesh generator, no CL-mesh. The tool-centre surface exists only implicitly as *the drop-cutter result sampled on a grid*, and the codebase says so explicitly:

- `crates/rs_cam_core/src/tier_map.rs:78` — *"The drop-cutter Z is the tool-CENTRE offset surface, not the machined surface"*, followed (`:79-110`) by the `bias_k(θ) = (R_k − R_finest)·(sec θ − 1)` slope-bias analysis and `ResidualTreatment::SlopeCompensated`.
- `crates/rs_cam_core/src/finish_setup.rs:258-292` — `FinishSurfaceKind::CutterOffset` = *"tool-centre (CL) offset surface the tool would ride"*, described at `:281` as `"generation (cutter CL offset surface)"`.
- `crates/rs_cam_core/src/finish_setup.rs:495-512` — documented limitation: *"a ball tool's offset surface geometrically hides steepness … only 0.1% of the offset surface reads that steep, with a max of 52° vs a [true] …"*.
- `crates/rs_cam_core/src/classify_probe.rs:167-173` — `ClassificationSampler::samples_probe_cl_surface()`; `:809` asserts `PRODUCTION` does **not**.

**Drop-cutter lives at `crates/rs_cam_core/src/dropcutter.rs`:**
- `point_drop_cutter<C: MillingCutter + ?Sized>(...)` (`:16`) — single Z-down projection of a cutter onto the mesh at one XY.
- `DropCutterGrid` (`:58`) with `get(row, col) -> &CLPoint` (`:76`) and a `points: Vec<CLPoint>` field.
- `batch_drop_cutter` (`:89`) / `batch_drop_cutter_with_cancel` (`:110`) — builds an axis-aligned (optionally rotated) lattice of CL points.
- `point_is_over_mesh_xy` (`:237`).

It projects **only onto `TriangleMesh` triangles, Z-down (or Z-up via `ProjectDirection::FromBelow` in `project_curve.rs:22`)**, dispatched through the `MillingCutter` trait's `vertex_drop` / `facet_drop` / `edge_drop` / `drop_cutter` (`crates/rs_cam_core/src/tool/mod.rs:429, 440, 503, 506`) with `center_height` / `normal_length` / `xy_normal_length` (`:424-426`). `BallEndmill` implements these at `crates/rs_cam_core/src/tool/ball.rs:86-96`.

**The lateral counterpart is `pushcutter.rs`** (`push_cutter_triangle`, `:17`) — horizontal cutter push along a `Fiber` at constant Z, used by `waterline.rs`. Same three contact types (vertex/facet/edge), still not an offset surface.

**Verdict:** drop-cutter (Z-axis) + push-cutter (horizontal fiber) projection is the *only* cutter-centre mechanism. A genuine 3D offset/inflated surface, or an offset surface parameterised in mesh UV, is **MISSING**.

---

## 3. Region boundaries with holes — EXISTS; but the Wanaka region-1 capture is NOT durable

**Polygon type — EXISTS with holes.** `crates/rs_cam_core/src/polygon.rs:113`
```rust
pub struct Polygon2 { pub exterior: Vec<P2>, pub holes: Vec<Vec<P2>>, pub closed: bool, bbox_cache: OnceLock<[f64;4]> }
```
`with_holes` (`:150`), `with_holes_closed` (`:163`), `open_path` (`:140`, `closed: false` — used for contour lines/rivers). `contains_point` (`:295`) honours holes; `contains_point_eps` (`:313`). Booleans via `geo` 0.32: `union`/`intersection`/`difference`/`union_all` (`:350`, `:367`, `:378`, `:393`); `to_geo_polygon`/`from_geo_polygon` (`:253`, `:260`). Nesting reconstruction: `detect_containment(Vec<Polygon2>) -> Vec<Polygon2>` (`:1537`).

**Multi-polygon set — EXISTS.** `RegionSet<'a>` (`crates/rs_cam_core/src/region_set.rs:29`): `new` (`:35`), `from_slice` (`:44`), `contains(&P2)` (`:71`), `single_union` (`:80`), `processed(keep_outs, offset)` (`:92`). This is the type every generator takes as its machining boundary.

**Mask → polygon-with-holes extractor — EXISTS.** `crates/rs_cam_core/src/region_mask.rs`: `region_polygons_from_mask` (`:111`), `region_polygons_from_mask_clamped` (`:150`), `region_polygons_from_mask_reported` (`:169`), with `RegionExtraction` (`:78`) and `RegionCapReport` (`:47`). Dilation by `overlap_mm` via a whole-grid EDT, then marching squares (`contour_extract::marching_squares_bool_grid`, `region_mask.rs:237`), then containment grouping.

**Wanaka region-1 boundary capture — NO CHECKED-IN FIXTURE. This is a gap the programme's "shared experimental contract" depends on.**

The boundary exists only *transiently*, recomputed inside one `#[ignore]`d evidence test:
- `crates/rs_cam_core/tests/thin_organic_island_widths.rs:99` — `const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";` — 661,212 triangles, **not in the repo**; the test SKIPs (does not fail) when absent.
- The chain that produces region 1 is `wanaka_monotone_cells_kept_retracts` (`:552`): `TriangleMesh::from_stl_scaled` → `SpatialIndex::build_auto` → two `TaperedBallEndmill`s (R1.5: `(3.0, 2.8, 6.0, 30.5)`; R1.0: `(2.0, 5.7, 6.0, 20.0)`) → `compute_tier_map` with `ResidualTreatment::SlopeCompensated` → `extract_tier_islands` (tier 1) → `build_classification_surface_with_sampler_and_cancel` → `finish_planner::decompose` → `planned.regions` → `stage_i` (`:1677`) → `stage_j` (`:1963`) / `stage_k` (`:2077`). Region "1" is just `zero_regions.first()` (`:2088`).
- The only artifacts are SVGs written to `THIN_ORGANIC_SVG_DIR` or `target/thin_organic/` (`cell_svg_output_dir`, `:1733`; `write_cell_svg_comparisons`, `:1768`; `write_named_cell_svg`, `:1810`) — rendering, not a reloadable boundary.
- `fixtures/` and `test_data/` contain **no** wanaka artifact (only `terrain_small.stl`, `ux_3d_terrain.toml`, `m5_terrain_mid_steep_polygon.json`, etc.).

**Flag:** the programme states *"Every arm starts from the same captured Wanaka region-1 boundary."* There is no such capture today. Either F0 must add a serialisation step (there is precedent — `test_data/m5_terrain_mid_steep_polygon.json`, `test_data/cavalier_panic_polygon_r1.json`), or every arm re-derives it from a machine-local 661k-triangle STL and inherits the whole tier-map/decompose pipeline as an uncontrolled input.

**Tapered-ball, not ball-end.** The evidence used `TaperedBallEndmill`, matching the programme's own caveat at PROGRAMME.md:52-53.

---

## 4. Sparse linear solve — **MISSING. A NEW DEPENDENCY IS REQUIRED.**

Exhaustive manifest sweep (all 6 `Cargo.toml` in the repo):

| manifest | linear-algebra deps |
|---|---|
| `Cargo.toml` (workspace) | `nalgebra = "0.33"` only. Also `kiddo = "4"`, `geo = "0.32"`, `cavalier_contours = "0.7"` |
| `crates/rs_cam_core/Cargo.toml` | `nalgebra = { workspace = true }` |
| `crates/rs_cam_viz/Cargo.toml` | `nalgebra = { workspace = true }` |
| `crates/rs_cam_cli/Cargo.toml` | none |
| `crates/rs_cam_mcp/Cargo.toml` | none |
| `tests/step_validation/Cargo.toml` (workspace-**excluded**) | truck-* only |

`Cargo.lock` grep for `faer|sprs|russell|nalgebra-sparse|ndarray|argmin`: **only `nalgebra` (line 2689) matches.** No sparse crate is anywhere in the tree, transitively or directly.

**`nalgebra` 0.33 is dense-only** — `nalgebra-sparse` is a separate crate and is not present. There is **no** sparse matrix type, no CG/Cholesky/LU sparse factorisation, no Poisson/Laplacian assembly anywhere.

Existing solver-ish usage (all dense/closed-form, none reusable for a Laplacian):
- `crates/rs_cam_core/src/arcfit.rs:328, 550-554` — `circle_from_least_squares`, Kåsa algebraic 3×3 LSQ circle fit. Documented bias at `:397`: *"biased toward huge [radii]"*.
- `crates/rs_cam_core/src/crest_lines.rs:49` — `nalgebra::{Matrix3, Matrix4, Vector3, Vector4}` for the 3×3 curvature-tensor eigen-decomposition and 4-vector derivative tensor, per vertex. Dense, tiny, local.
- `crates/rs_cam_core/tests/thin_organic_island_widths.rs:1636` — `pca_minor_and_elongation`, a hand-rolled 2×2 second-moment eigen-solve (test-local).
- `crates/rs_cam_core/tests/rubbing_floor_diameter_scaling_measurement.rs:786` — OLS slope on log-log (test-local).

**Verdict for the design note:** the Zou/Wang/Feng arm's Poisson formulation, and any cotangent-Laplacian conformal map for the Shen et al. arm, **cannot** be built from what is in the tree. Either add a dependency (`faer`, `sprs`, `nalgebra-sparse`, `russell_sparse`) or hand-roll an iterative solver.

**Precedent worth citing:** `crates/rs_cam_core/src/scallop_isofield.rs:64-70` faced the same choice for its Eikonal PDE and solved it *matrix-free* — Gauss–Seidel **fast sweeping** with Godunov upwind updates, chosen because it *"needs no heap, is trivially deterministic, and converges in a fixed small number of sweeps."* A matrix-free Jacobi/Gauss–Seidel or CG Laplacian on the same grid would need no new dependency; a *mesh-domain* cotangent Laplacian almost certainly would.

Also note `kiddo = "4"` is pinned in `[workspace.dependencies]` but **no member crate lists it** — it is dead in the manifest and absent from every `use`. Relevant only as a free option for spacing/nearest-neighbour queries if a streamline arm needs one.

---

## 5. UV / conformal parameterisation — **MISSING (nothing at all)**

Repo-wide grep for `conformal|parameteri[sz]ation|uv_map|\buv\b|slit map|LSCM|ABF|harmonic map` across `crates/`:
- `crates/rs_cam_core/src/tool_shape_key.rs:33` — "parameterisation" of *tool geometry*, unrelated.
- `crates/rs_cam_core/src/compute/sim_prefix.rs:333` — same, tool `geometry_hint()`.
- `crates/rs_cam_core/src/tool/vbit.rs:294` — "the parameterization from OpenCAMLib" for a V-bit profile, unrelated.
- `crates/rs_cam_core/src/crest_lines.rs:505-512` — local variables literally named `u`, `vv`, `uv` inside the curvature-tensor accumulation. **Not a surface parameterisation.**
- `crates/rs_cam_core/src/enriched_mesh.rs:26-38` — `SurfaceType` / `SurfaceParams` carry analytic STEP surface *type* (plane/cylinder/etc.) from `truck`, not a UV chart, and only for STEP imports.

There is **no** UV chart, no texture coordinate storage, no cotangent weights, no boundary-mapping, no cut-graph, no slit map, no genus/topology analysis. Both papers' parameterisation steps are 100% greenfield.

---

## 6. Streamline / iso-curve tracing — 2D grid iso-curves EXIST; **on-mesh direction-field streamlines MISSING**

**Marching squares (2D grid) — EXISTS, well factored.** `crates/rs_cam_core/src/marching_squares.rs`: `cell_case(bl,br,tr,tl) -> u8` (`:94`), `cell_segments(case) -> &'static [(u8,u8)]` (`:124`, saddle cases bridge the true diagonal — test at `:257`), `chain_segments(&[(P2,P2)]) -> Vec<Vec<P2>>` (`:150`), `CHAIN_EPS`. It operates on **boolean/scalar values at the four corners of a regular 2D cell** — nothing mesh-aware.

Consumers:
- `crates/rs_cam_core/src/contour_extract.rs:241` — `marching_squares_bool_grid(grid, rows, cols, origin_x, origin_y, cell)`; boolean occupancy → closed loops in world XY.
- `crates/rs_cam_core/src/region_mask.rs:237` — mask → region polygons with holes.
- `crates/rs_cam_core/src/boundary.rs:407, 495` — `marching_squares_grid`, used by `model_silhouette` (`:385`).
- `crates/rs_cam_core/src/scallop_isofield.rs:72, 428` — `iso_contour` at a real-valued level (its own float-crossing variant, differing from the bool version, documented at `:422`).

**Iso-scallop level-set extraction — EXISTS as a research module.** `crates/rs_cam_core/src/scallop_isofield.rs` (599 lines). This is the closest existing thing to F1's requirement, and its header (`:1-70`) is worth reading verbatim for F0. It solves
```
|∇D| = 1 / s(x, y),   D = 0 on the region boundary
```
by **fast sweeping with the Godunov upwind update** on a 2D grid, and extracts the integer level sets as passes. API: `IsoScallopField` (`:80`), `build_field(...)` (`:122`), `extract_rings(&field) -> Vec<Vec<P2>>` (`:405`), `iso_contour(&field, level)` (`:428`); internals `distance_to_boundary` (`:188`), `seed_boundary` (`:244`), `fast_sweep` (`:307`), `neighbour_min` (`:372`).

Stated limitations (from the module's own header):
- **Not on a production path** — reachable only via `crate::scallop::RingSource::IsoField`, *"which no shipped caller selects"* (`:6-8`).
- Rings are **grid contours**, so XY placement carries the cell size as an error floor, where an offset ring is exact in XY (`:52-55`).
- The Eikonal solve is `O(cells)` per sweep × several sweeps — *"real work the cascade does not do"* (`:56-57`).
- *"A saddle in `D` merges or splits level sets. Marching squares handles that correctly and silently, which is a feature for coverage and a hazard for anything downstream that assumes ring identity."* (`:58-60`) — directly relevant to F1's singularity/termination reporting requirement.
- Its stepover field is derived from `SlopeMap` only; it does **not** accept an arbitrary preferred direction field.

**On-mesh scalar iso-curve tracing across triangles — EXISTS, in exactly one place.** `crates/rs_cam_core/src/crest_lines.rs:588-660`, `march_valley_segments`: for each triangle it orients t₂ consistently against vertex 0's direction, flips e₂ signs to match, finds the two edges where e₂ changes sign, linearly interpolates the crossing point, and emits a segment keyed by `EdgeKey`; then chains coincident edge-crossings into polylines. That is *marching-triangles* on a per-vertex scalar, with a per-vertex direction field used for sign disambiguation — genuinely the mechanism a direction-field arm needs. **But:** it is private, hard-wired to the valley predicate (`concave && dominant && sharp`, `:649-652`), silently drops any triangle with `crossings.len() != 2` (`:640` — no saddle handling), and has no generic "trace level set of arbitrary field φ" entry point.

**Streamline integration of a direction field — MISSING.** No RK/Euler integration on a mesh or grid, no seeding/spacing/termination policy, no streamline-placement algorithm anywhere in the tree.

**Geodesic — only in the grid-Eikonal sense** (`scallop_isofield`'s `D`). No exact/heat-method geodesic on a mesh, no `MMP`/`heat method`/`Dijkstra-on-mesh`.

**Other on-surface curve machinery (all drop-cutter based, not surface-intrinsic):**
- `crates/rs_cam_core/src/project_curve.rs:1-5` — resamples a `Polygon2`'s rings and drops each point onto the mesh; `ProjectDirection` (`:22`), `ProjectSide` (`:31`), `ProjectCurveParams` (`:43`). Projects **onto the `TriangleMesh` via `point_drop_cutter`**, i.e. it produces a CL curve, not a surface curve.
- `crates/rs_cam_core/src/contour_extract.rs:34` — `weave_contours(x_fibers, y_fibers, z)`, waterline Z-contours from push-cutter fibers.
- `crates/rs_cam_core/src/spiral_finish.rs:1-13` — Archimedean spiral `r(θ) = stepover·θ/(2π)` in **XY only**, drop-cut per point. Explicitly a planar spiral, not a surface spiral; no topology awareness. Do not mistake this for a conformal spiral.
- `crates/rs_cam_core/src/pencil.rs:379-402` — Laplacian *smoothing* of a polyline (1D, neighbour-midpoint). Not a Laplace solve.
- `crates/rs_cam_core/src/grid_field.rs` — `distance_transform_2d` (`:79`), `edt_curvature_field` (`:128`), `smooth_grid` (`:181`).

---

## 7. Robust curve clipping — EXISTS, mature, with documented failure containment

**Offsetting: `cavalier_contours` 0.7.** Chokepoint is deliberately singular — `crates/rs_cam_core/src/polygon.rs:571` (*"The single chokepoint where cavalier_contours' offset is called"*). Public API: `offset_polygon(polygon, distance) -> Vec<Polygon2>` (`:533`), `offset_polygon_reported(...)` (`:551`) returning `(Vec<Polygon2>, Option<OffsetFailure>)`. `OffsetFailure` (`:425`) distinguishes `LibraryFailure` (cavalier or a transitive dep panicked and was contained, `:429`) from this module's own rejections (`OffsetRejection`, `:456`); `describe()` at `:479`. Multi-ring: `OffsetRingSet` (`:1405`) with `from_polygon`/`from_polygons`/`offset`/`offset_reported`/`offset_per_group`/`to_polygons(FlattenPolicy)` (`:1413-1521`). `FlattenPolicy::from_chord_tolerance` (`:1114`) — arcs are preserved through offsetting and flattened at the end.

Known limitations, in comments: `polygon.rs:35` — the failures *"are `debug_assert!`s inside `cavalier_contours` and its transitive [deps]"*; `polygon.rs:449` — NaN still reaches the library (F-11/F-13); root `Cargo.toml` release profile deliberately keeps `panic = "unwind"` *because* `polygon::offset_polygon`'s cavalier containment depends on it. Regression tests: `crates/rs_cam_core/tests/offset_polygon_degenerate_inputs_r1.rs`, `tests/adversarial_2d_campaign_r2.rs:371`, `tests/unified_finish_ring_collapse_g_unifiedcrash.rs`.

**Booleans: `geo` 0.32** via `Polygon2::union`/`intersection`/`difference`/`union_all` (`polygon.rs:350-393`), `repaired()` (`:339`), `has_self_intersection()` (`:326`). Simplification: `cleanup_collinear` (`:900`), `simplify_bounded` (`:951`).

**Toolpath containment — `ToolContainment` at `crates/rs_cam_core/src/boundary.rs:15`:**
- `Center` / `Inside` / `Outside`; `Inside` → `offset_polygon_reported(boundary, +tool_radius)` (`:75`), `Outside` → `-tool_radius` (`:77`).
- `effective_boundary(boundary, containment, tool_radius) -> Vec<Polygon2>` (`:36`); `effective_boundary_reported` (`:66`).
- Clipping: `clip_toolpath_to_boundary(tp, boundary, safe_z) -> Toolpath` (`:156`), `clip_annotated_to_boundary_set(...)` (`:175`), plus provenance-tracking variants (`:249`, `:275`).
- `apply_user_boundary_offset` (`:125`) with `UserOffsetOutcome` (`:100`); `subtract_keepouts` (`:142`); `model_silhouette(mesh, cell_size)` (`:385`).
- Production wiring: `crates/rs_cam_core/src/session/compute.rs:2060, 2097-2101, 2243, 2252-2256` maps `compute::config::BoundaryContainment` → `ToolContainment`.
- Escape tests: `crates/rs_cam_core/tests/boundary_clip_escape_f1.rs`, `tests/adaptive3d_boundary_clear_parity.rs`, `tests/project_curve_deviation.rs`.

This is the strongest primitive in the inventory. F1's "prove every emitted segment is inside the region" requirement is directly served by `clip_annotated_to_boundary_set` + `RegionSet::contains`.

---

## 8. F-034 time integrator + relink harness — EXISTS (integrator is library; **the costing harness is test-local**)

**Integrator — library API, `crates/rs_cam_core/src/machine_kinematics.rs`:**
```rust
pub fn compute_cycle_time(toolpath: &Toolpath, kinematics: &MachineKinematics,
                          max_feed_mm_min: f64, rapid_feed_mm_min: f64) -> f64   // :353
pub fn compute_cycle_time_breakdown(...) -> CycleTimeBreakdown                    // :460
```
Trapezoidal per-move profile with junction-velocity lookahead (documented `:335-352`); solves the triangular profile when a move is too short to reach `v_cmd`. `CycleTimeBreakdown` (`:427`) buckets by `MoveIntent`, disjoint, summing bit-identically to `total_s`; has an honest `unknown_s` bucket for untagged legacy generators. Also `retract_link_time` (`:372`), `surface_link_time` (`:401`), `LinkKinematics` (`:328`), `MachineKinematics` (`:60`) with `shapeoko_xxl_stock` (`:156`) / `shapeoko_xxl_ricky_tuned` (`:202`) / `effective_accel` (`:221`) / `from_grbl_settings` (`:262`). Also `predicted_feeds_for_toolpath` (`:636`), `predicted_achieved_feed` (`:732`).

**Relinker — library API, `crates/rs_cam_core/src/surface_link.rs`:**
```rust
pub fn relink_fragments(AnnotatedToolpath, &TriangleMesh, &SpatialIndex,
                        &dyn MillingCutter /*generic*/, &RelinkParams) -> (AnnotatedToolpath, RelinkReport)  // :354
```
with `RelinkParams<'a>` (`:191`), `RelinkReport` (`:290`), `LinkCeiling<'a>` (`:81`), `build_surface_link` (`:24`).

**The costing harness the programme means is `relink_and_cost` — TEST-LOCAL, not library code.** `crates/rs_cam_core/tests/thin_organic_island_widths.rs:1862`:
```rust
fn relink_and_cost(raw: Toolpath, mesh: &TriangleMesh, index: &SpatialIndex,
                   cutter: &TaperedBallEndmill, boundary: &RegionSet<'_>,
                   kinematics: &MachineKinematics, safe_z: f64) -> CandidateCost
```
`CandidateCost` (`:1853`) = `{ moves, cutting_mm, time_s, fragments, linked, kept_retracts }`.

**What a candidate must provide: a raw `rs_cam_core::toolpath::Toolpath`** — nothing more. Not a list of polylines, not a strategy object. The harness then:
1. wraps it in `toolpath_spans::AnnotatedToolpath::new(raw)`;
2. runs `surface_link::relink_fragments` with `RelinkParams { hookup_distance: 25.0, stock_to_leave: 0.0, sampling: 0.5, feed_rate: 735.0, plunge_rate: 180.0, safe_z, link_kinematics: Some(&LinkKinematics{…}), reorder: true, boundary: Some(&RegionSet), link_ceiling: None, flush_ride: false, airborne_links_may_leave_territory: false }` (`:1878-1891`);
3. reconciles through `transform_provenance::ReconcileSet::new(None, None)` (`:1899`);
4. costs with `compute_cycle_time`.

The reference candidate builder is `raster_candidate(grid, regions, safe_z, effective_min_z) -> Toolpath` (`:1911`), which loops regions calling `toolpath::raster_toolpath_from_grid(grid, feed, plunge, safe_z, Some(min_z), Some(&RegionSet))` and concatenates `.moves`. A new arm would replace exactly this function.

Two hard constraints a new arm must honour:
- `cell_membership_matches` (`:1936`) is a **refusal gate**: it walks every `grid.points` and refuses to publish a comparison if the candidate's region membership disagrees with the baseline on even one emitted point. A candidate that changes coverage cannot be costed against the baseline.
- `link_ceiling: None` (`:1888`). PROGRAMME.md:66-68 permits this **only** for the first fresh-stock geometry experiment and requires labelling. The 875.9 s number therefore carries that caveat; any promising candidate must re-run with the corrected rest-stock ceiling before a time claim.

Machine pin (`:657-664`, from `wanaka200_mt2.toml`): accel XYZ `[500, 500, 270]`, scalar `423.333…`, junction deviation `0.02 mm`, max feed `10000`, rapid `5000`, feed `735`, plunge `180`.

Other F-034 sites: `crates/rs_cam_core/tests/machine_kinematics_cycle_time_f034.rs` (the sentry, `heavy-tests`-gated, 91 s), `tests/predicted_feed_gates_f035.rs`, `tests/feed_modulation_cycle_time_f036c.rs`, `tests/scallop_intra_pass_relink_am7.rs`, `tests/p1_headless_ab_wanaka.rs`, `crates/rs_cam_cli/src/nc_replay.rs`. **Timing caveat in the root `Cargo.toml`:** F-034/F-036c anchors are calibrated on the real `release` profile — not `release-fast`, and were pinned under `panic="abort"`.

---

## 9. Scallop-height / stepover-from-cusp — EXISTS

**Formula module: `crates/rs_cam_core/src/scallop_math.rs`**
- `scallop_height_flat(tool_radius, stepover)` (`:15`)
- `stepover_from_scallop_flat(tool_radius, scallop_height)` (`:30`) — the inverse; `s = 2·√(2Rh − h²)`
- `effective_radius(tool_radius, curvature_radius)` (`:47`)
- `stepover_from_scallop_curved(tool_radius, scallop_height, curvature)` (`:75`)
- `scallop_height_curved(tool_radius, stepover, curvature)` (`:85`)
- `variable_stepover(...)` (`:143`) — the **shipped** law, slope- and curvature-aware

**Production equal-cusp site: `crates/rs_cam_core/src/session/multitool.rs:122`** — `pub fn equal_cusp_stepover_mm(cusp_radius_mm, cusp_height_mm) -> f64`; rationale at `:53` (*"At h = 30 µm that is 0.597 mm for an [R1.5]"*), used at `:695`, pinned by tests at `:1044-1054` (`equal_cusp_stepover_mm(1.5, 0.03) == 0.596_992_462`, `(2.0, 0.03) == 0.690_217_357`). The evidence test restates it locally at `tests/thin_organic_island_widths.rs:124` *"because an instrument should show its own arithmetic"* — and its constant `CUSP_HEIGHT_MM = 0.03` (`:117`) matches.

**Scallop operation: `crates/rs_cam_core/src/scallop.rs`** (3341 lines). Offset-cascade architecture, documented at `:1-16`: project silhouette → `variable_stepover` → iterative `offset_polygon` inward → drop-cutter Z per ring point → chain. Uses `variable_stepover` (`:26`, `:359`), `stepover_from_scallop_flat` (`:227`), `stepover_from_scallop_curved` (`:394`). `RingSource::IsoField` is the (unshipped) alternate backend from §6.

Known limitations recorded in M4/M5 tests: `tests/scallop_oracle_validation_m4.rs:25, 467, 550` — *"`variable_stepover` scales the stepover the OTHER [way]"* / it uses `1/√cos θ`; `tests/scallop_candidates_m4.rs:529-533`; `tests/scallop_isofield_gouge_m4.rs`; the `max_rings` cap critique at `scallop_isofield.rs:39-45`. Also `crates/rs_cam_core/src/slope.rs:475` documents a clamp callers must apply before `variable_stepover`.

`MillingCutter::cusp_radius` / `cusp_radius_mm` (`crates/rs_cam_core/src/tool/mod.rs:295, 311`) supply the R for a tapered-ball or ball tool.

---

## 10. PCA-cell / monotone-cell evidence harness — EXISTS (one file)

**The instrument: `crates/rs_cam_core/tests/thin_organic_island_widths.rs` (2207 lines).** Commit lineage:
- `9f5b4e2a` "evidence(finish): compare rotated monotone cells" — +219 lines to this test, +44 to FINDINGS.md → **this is the commit that produced 875.9 s** (Stage K).
- `177d8aca` "evidence(finish): render monotone cell overlays" — +87 lines, the SVG overlays.
- `f10bf715` "docs(finish): charter conformal finishing feasibility" — created PROGRAMME.md + CREDITS.md entries only.

**The evidence doc: `planning/thin_organic_2026-08-27/FINDINGS.md`.** The numbers:
- `FINDINGS.md:290` — `| 3104 mm² | 1053.7 s | 1203.3 s | 0.88× |` (the 0° undivided baseline).
- `FINDINGS.md:361, 376, 428` — angle sweep and PCA prediction (region 1 elongation **4.15**, PCA minor **119.6°**).
- `FINDINGS.md:502` — Stages I/J: `| 1 | 3104 mm² | 86 | 97 | 83 | 1053.7 → 999.9 s |`.
- `FINDINGS.md:540-566` — **§0h "Rotated-cell precursor (2026-08-28)"**, the table PROGRAMME.md quotes:

| candidate, region 1 | cells | fragments | kept retracts | F-034 time | cutting distance |
|---|---|---|---|---|---|
| 0° undivided | — | 564 | 97 | **1053.7 s** | 8687 mm |
| PCA-minor undivided | — | 491 | 74 | 963.0 s | 8501 mm |
| **PCA-minor cells** | **69** | **141** | **53** | **875.9 s** | **8279 mm** |

  (SVG artifacts named at `:565-566`: `wanaka_monotone_cells_region_1.svg`, `wanaka_monotone_cells_region_1_pca_minor.svg`.) Also `FINDINGS.md:579` names this the precursor to the direction-field prototype; `:795-820` sketch the productionisation (`finish_planner.rs:235-238`, `PlannedRegion` gaining `sweep_deg`).

**Test entry points:**
- `wanaka_tier_and_band_region_widths` (`:259`, `#[ignore]`) — historic Stages A–H, 12-angle sweep.
- `wanaka_monotone_cells_kept_retracts` (`:552`, `#[ignore]`) — **the one that produces the 875.9/1053.7 comparison.** Invocation documented at `:56-65`:
  ```
  THIN_ORGANIC_SVG_DIR=/home/ricky/Downloads/svg \
  cargo test -p rs_cam_core --test thin_organic_island_widths \
    wanaka_monotone_cells_kept_retracts -- --ignored --nocapture
  ```

**Supporting machinery in that file (reusable by an F1 arm, but test-local):**
- `RegionCells { boundary, cells, topology_cells }` (`:1458`), `GridRun` (`:1464`)
- `grid_for_direction(mesh, index, cutter, stepover, angle_deg)` (`:1470`) — **rotated drop-cutter lattice**; `grid_for_stage_i` (`:1487`)
- `runs_in_row` (`:1496`), `runs_overlap` (`:1513`), `grid_frame_to_world` (`:1517`)
- `polygons_for_lattice_cell` (`:1526`), `lattice_boustrophedon_cells(grid, boundary, min_z)` (`:1571`) — the Choset-style monotone decomposition, emitted-lattice-based
- `pca_minor_and_elongation(poly, cell) -> Option<(f64, f64)>` (`:1636`) — hand-rolled 2×2 second-moment eigen
- `stage_i` (`:1677`), `stage_j` (`:1963`), `stage_k` (`:2077`)
- `write_named_cell_svg` (`:1810`), `write_cell_svg_comparisons` (`:1768`), `svg_path` (`:1746`), `cell_svg_output_dir` (`:1733`)
- Width measurement: `polygon_width_mm` (`:174`, EDT inscribed diameter, hole-honouring), `components` (`:132`), `percentile` (`:200`), `report_widths` (`:209`)

**Limitation stated in the file itself** (`:67-70`): `#[ignore]` because it needs the operator's Wanaka mesh, which is not in the repo; it **SKIPs rather than fails** when absent. Stage K's own closing note (`:2202-2205`): *"Read the two within-direction A/Bs first… The 0° ↔ PCA rows deliberately change the raster lattice and require C4 surface review."*

---

# Summary table

| # | Primitive | Verdict |
|---|---|---|
| 1 | Mesh storage, normals, spatial index | **EXISTS** (`mesh.rs`, `geo.rs`) |
| 1 | Barycentric interpolation as a primitive | **MISSING** (two ad-hoc sites only) |
| 1 | Half-edge / triangle-neighbour walking | **MISSING** (only edge→face map, `pencil_dihedral.rs`) |
| 1 | Per-vertex normal/curvature API | **MISSING as public API** (private in `crest_lines.rs`) |
| 2 | Offset / inflated cutter-centre **surface** | **MISSING** — drop-cutter (+push-cutter) grid projection is the only mechanism |
| 3 | Polygon with holes, multi-region set, mask→polygon | **EXISTS** (`polygon.rs`, `region_set.rs`, `region_mask.rs`) |
| 3 | Durable captured Wanaka region-1 boundary | **MISSING** — recomputed transiently from a machine-local STL |
| 4 | Sparse linear solve | **MISSING — NEW DEPENDENCY REQUIRED** (only dense `nalgebra` 0.33 in the entire lockfile) |
| 5 | UV / conformal parameterisation | **MISSING — nothing at all** |
| 6 | 2D grid iso-curves (marching squares) | **EXISTS** (`marching_squares.rs` + 4 consumers) |
| 6 | Eikonal / iso-scallop level sets on a 2D grid | **EXISTS, research-only** (`scallop_isofield.rs`, unshipped) |
| 6 | On-mesh scalar iso-curve tracing across triangles | **EXISTS but private + valley-specific** (`crest_lines.rs:588`) |
| 6 | Direction-field streamline integration | **MISSING** |
| 6 | Mesh geodesics | **MISSING** (grid-Eikonal only) |
| 7 | Polygon offsetting + booleans + toolpath clipping | **EXISTS, mature** (`cavalier_contours` 0.7, `geo` 0.32, `boundary.rs`) |
| 8 | F-034 integrator | **EXISTS, library** (`machine_kinematics.rs:353`) |
| 8 | Relink+cost candidate harness | **EXISTS but TEST-LOCAL** (`thin_organic_island_widths.rs:1862`); input = a raw `Toolpath` |
| 9 | Cusp→stepover formulas + scallop op | **EXISTS** (`scallop_math.rs`, `session/multitool.rs:122`, `scallop.rs`) |
| 10 | PCA-cell / monotone-cell evidence harness | **EXISTS** (`thin_organic_island_widths.rs`, Stages I/J/K; `FINDINGS.md` §0h) |

**Three findings the design note must not lose:**

1. **Sparse solve is the only hard dependency gap.** Both papers need one; nothing in the tree provides it. `scallop_isofield.rs`'s matrix-free fast-sweeping is the in-repo precedent for avoiding it on a *grid*, but a mesh-domain Poisson/cotangent-Laplacian cannot be avoided that way.
2. **There is no cutter-centre surface object.** Every "offset surface" in the codebase is a drop-cutter heightmap on a regular XY grid, with a documented slope bias (`tier_map.rs:79-90`) and a documented steepness-hiding artifact (`finish_setup.rs:495-512`). A ball-end conformal spiral wanting a true CL surface has nothing to build on.
3. **The shared experimental contract has no artifact.** "The same captured Wanaka region-1 boundary" does not exist as a file; it is a function of a 661k-triangle STL outside the repo plus the entire tier-map→islands→classify→decompose pipeline. Serialising it should probably be F0's first concrete deliverable.
