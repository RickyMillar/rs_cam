# T1 — Reachability math: "does an R-size ball fit here?"

Track T1 of the multi-tool island-finishing investigation, 2026-08-23.
Read-only survey; nothing was built, benchmarked or changed. Every claim
carries a `file:line`. Timing figures are **quoted from committed
measurement records**, never re-run here — where I extrapolate, the
extrapolation is labelled as such and its arithmetic is shown.

Board of record: wanaka200 — **stock** 240 × 250 × 25 mm white oak
(`planning/airrun_2026-08-19/wanaka200_p2.toml:8-10`), **terrain mesh**
`/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl`
(`wanaka200_p2.toml:216`) measuring **200.00 × 200.00 × 9.81 mm** with
**661,212 triangles** — read directly off the binary STL header and vertex
stream, and matching the "661 k-triangle terrain" the repo names at
`crates/rs_cam_core/src/geom_cache.rs:6` and
`crates/rs_cam_core/src/mesh.rs:724-728`. **Every drop-cutter grid in this
codebase is sized by the mesh bbox, not the stock** — see §1.3.

---

## 0. One-paragraph answer

The primitive the investigation is looking for **already exists and is
already shipped**, twice: `rest_field::detect_rest_valleys` computes a
per-cell residual between two arbitrary cutters
(`crates/rs_cam_core/src/rest_field.rs:616-683`), thresholds it into an
island mask (`:722`), and extracts island polygons
(`:830-836` → `crates/rs_cam_core/src/region_mask.rs:88-207`); and
`compute::execute::attach_generic_rest_analysis`
(`crates/rs_cam_core/src/compute/execute.rs:3023-3091`) runs that detector
for **any** operation family with **any** configured reference tool, hanging
the result on the toolpath so a downstream op can be confined to it via
`BoundarySource::DerivedRestRegions`
(`crates/rs_cam_core/src/compute/config.rs:1454-1456`). The July "P2 selective
finishing" commit `2195d33` wired all of that end to end and **measured it on
this exact board: a selective fine scallop cut 1593 mm against a 7788 mm
all-over baseline, ~80 % less** (§4.1). What is missing is not the machinery
but (a) an *n*-tool pass instead of a 2-tool one, (b) a planner that picks the
tiers and can say "bigger is strictly faster here at equal cusp", and (c) an
honest handling of the systematic **slope bias** in the tool-vs-tool CL
difference (§3.4), which is the single biggest technical risk in the whole
idea.

---

## 1. Where drop-cutter lives, and what a full-grid per-tool map costs

### 1.1 The machinery

| layer | location | shape |
|---|---|---|
| single point | `crates/rs_cam_core/src/dropcutter.rs:16-43` `point_drop_cutter` | one `index.query(x, y, cutter.radius())`, then `drop_cutter_can_contact` prune, then `cutter.drop_cutter(&mut cl, tri)` per surviving triangle |
| one grid cell (+ optional coverage) | `crates/rs_cam_core/src/dropcutter.rs:252-267` `sample_grid_cell` | `point_drop_cutter` + `min_z` clamp + optional `point_is_over_mesh_xy` (a **second** index query) |
| batch grid | `crates/rs_cam_core/src/dropcutter.rs:282-341` `batch_sample_grid` | rayon over rows, cancel polled per row |
| raster grid product | `crates/rs_cam_core/src/dropcutter.rs:110-215` `batch_drop_cutter_with_cancel` → `DropCutterGrid` (`:57-72`) | axis-aligned or rotated sampling frame |
| **height field product** | `crates/rs_cam_core/src/slope.rs:87-234` `SurfaceHeightmap::from_mesh_with_cancel` | `z_values` + `covered` mask, 8192-cell cancel batches (`:193-207`) |
| **the per-tool surface** | `crates/rs_cam_core/src/finish_setup.rs:354-385` `build_finish_surface_with_policy_and_cancel` | takes `cutter: &dyn MillingCutter`, returns `FinishSurface` tagged `SurfaceSampler::CutterOffset` (`:383`) |
| **the true surface** | `crates/rs_cam_core/src/classify_probe.rs:248-264` `sample_classification_grid` | any of 5 arms; production is `TileRaster` (`:136`) |

The broadphase is a CSR uniform grid, `crates/rs_cam_core/src/mesh.rs:481-631`.
`build_auto` (`:499-517`) sizes the index cell as
`min(sqrt(8·area/tri_count), max_extent/50)` floored — i.e. **~8 triangles
per index cell by construction**. For wanaka: `sqrt(8·60000/661212)` =
**0.852 mm** index cells. (For the committed 100 × 100 mm / 215 k-triangle
`tests/fixtures/terrain.stl`, 0.61 mm — the study states that figure at
`planning/review_2026-07-29/CLASSIFICATION_PERF_STUDY.md:79`.)

### 1.2 The measured anchors — four of them, and none is a clean match

**No committed benchmark measures a board-scale full-grid drop-cutter sweep
with a production tool over a `build_auto` index.** That is a named gap. Four
committed measurements bracket it:

| # | source | measurement | implied rate |
|---|---|---|---|
| **A** | `planning/review_2026-07-29/RADIUS_AUDIT.md:83` (Q5) | **wanaka-native** classification sweep, 661 k-tri mesh, Ø0.05 probe, release: "1.0 s at 143², 9.6 s at 425², **47.3 s at 849²**, and 102.2 s at 1697²" | **~15 k cells/s wall** |
| **B** | `planning/review_2026-07-29/CLASSIFICATION_PERF_STUDY.md:192-200`, re-earned `:694-703` | 849² = **720,801 cells**, 215 k-tri / 100 mm fixture, Ø0.05 probe, 24-core: **2.99 s wall / 40.7 s CPU** (re-run 3.16 / 40.4) | **~241 k cells/s wall; 56 µs CPU/cell** |
| **C** | `planning/perf_review_2026-08-19/BASELINES.md:322-330` | G3 drop-cutter early-outs (`5efc1cbc`): `batch_drop_cutter/terrain_ball_6mm_step1` **79.099 → 13.001 ms (6.08×)**; `point_drop_cutter/terrain_center_ball` **6.3921 → 1.9588 µs (3.26×)** | **1.96 µs/drop serial**, Ø6.35 ball |
| **D** | same, `:326` / `TileRaster` row `CLASSIFICATION_PERF_STUDY.md:698` | true-surface sampler at 849²: **0.016 s wall / 0.13 s CPU** | ~45 M cells/s — effectively free |

Why they disagree by 16×: A and B differ only in mesh (661 k vs 215 k
triangles) and machine state, and 3.1× of triangles does not explain 15.8× of
time — **A predates both the G8 `build_auto` index-density fix and the G3
early-outs**, B is the study's own controlled run. C postdates both but is a
40,342-triangle / 100 × 73 mm fixture (`fixtures/terrain_small.stl`) indexed
at a **hand-set 10 mm cell** (`crates/rs_cam_core/benches/perf_suite.rs:37-45`)
— i.e. ~550 candidate triangles per query, grossly over-inclusive versus the
0.696 mm `build_auto` would choose. **C is pessimistic per query on a small
mesh; A is pessimistic per cell on the right mesh but on old code.**

One partial cancellation makes the mid estimate defensible: **the G3 early-outs
help most where there are many candidates to reject**, i.e. for a *real* tool,
and least for the Ø0.05 probe (1 index cell, ~8 candidates) that anchors A and
B. So the probe→real-tool penalty and the pre-G3→post-G3 gain pull in opposite
directions and substantially cancel.

Four facts that must not be lost:

1. **Two index queries per cell, not one** (`CLASSIFICATION_PERF_STUDY.md:66-72`):
   the contact query and the coverage query. A residual map needs the coverage
   mask exactly once, not once per tool.
2. **The per-query allocation is NOT the bottleneck** — pre-registered and
   refuted (`:88-113`). ~1/5 of the cost is querying, ~4/5 is the drop-cutter
   contact math (facet + 3 vertex + 3 edge drops per candidate triangle).
   `query_into`/`QueryScratch` exist (`crates/rs_cam_core/src/mesh.rs:740-756`)
   and bought 0.99–1.14× — do not plan around them.
3. **Rayon on this machine measures ~3.4× effective, not 24×**
   (`BASELINES.md:754-768`, measured on the sim kernel; no drop-cutter thread
   curve exists). Do not divide CPU-seconds by the core count.
4. **The whole-op effect is real**: making the classification grid 187× faster
   moved `UnifiedFinish` generation on the 100 mm terrain from **249.0 s →
   192.9 s, −22.5%** (`CLASSIFICATION_PERF_STUDY.md:643-646`).

**Scale context, corrected.** A full wanaka200 `generate_all` fixpoint (8 ops)
is **394 s (6 min 34 s)** at tip `33f2e022`
(`planning/perf_review_2026-08-19/BASELINES.md:610-615`) — the "~40 min"
figure carried in CLAUDE.md is explicitly retired there as *"approximate —
anecdotal wall clock, not a paired measurement"*. And the run is
**simulation-dominated**: whole-run CPU share is `query_model_z_range` 55.45 %,
`SpatialIndex::query` 23.21 %, `point_is_over_mesh_xy` 3.91 %,
`point_drop_cutter` **2.85 %** (`BASELINES.md:1198-1210`). Drop-cutter is
**not** where wanaka generation currently spends its time — which is exactly
why adding per-tool maps is a change in kind, not a marginal cost.

### 1.3 Extrapolation to the wanaka board — my arithmetic, shown

**First correction, and it matters: the grid is sized by the MESH, not the
stock.** Every drop-cutter grid builder pads the *mesh* bbox by one envelope
radius — `finish_setup.rs:366-372`, `classify_probe.rs:207-215`,
`dropcutter.rs:120` (`mesh.bbox.expand_by(r)`), `rest_field.rs:630-635`. The
wanaka **stock** is 240 × 250 (`planning/airrun_2026-08-19/wanaka200_p2.toml:8-9`)
but the terrain **mesh** is **200.00 × 200.00 × 9.81 mm** with **661,212
triangles** — measured directly off the binary STL header and vertex stream,
giving **16.5 tri/mm²** over 40,000 mm².

That makes the wanaka mesh remarkably close to the measured fixture
(`tests/fixtures/terrain.stl`, 214,997 triangles, 100 × 100 mm → 21.5
tri/mm²), and `build_auto` closes the remaining gap: wanaka's index cell is
`sqrt(8·40000/661212)` = **0.696 mm** against the fixture's 0.61 mm, both
carrying ~8 triangles per cell by construction (`mesh.rs:509-515`). **The
extrapolation is therefore over cell COUNT and tool RADIUS only** — the mesh
is not meaningfully denser or sparser than the one that was measured.

Grid cells over a 200 mm mesh padded by ~3 mm of envelope on each side
(206 mm span):

| spacing `c` | grid | cells | vs the measured 849² row |
|---|---|---|---|
| 0.6 mm | 344² | 118 k | 0.16× |
| **0.3 mm** | 688² | **473 k** | 0.66× |
| 0.15 mm | 1374² | 1.89 M | 2.6× |
| 0.125 mm (`cusp/4` for an R0.5 tip, `finish_setup.rs:165`) | 1649² | 2.72 M | 3.8× |

**Second correction — the one that dominates: a real tool is much wider than
the Ø0.05 probe the anchor used.** `point_drop_cutter` queries at
`cutter.radius()`, which is the **ENVELOPE** radius (`dropcutter.rs:29-30`,
and the hoist rationale at `:24-28`). On the shipped Ø1-tip / 7° / Ø6-shank
taper that is **3.0 mm**, not 0.5 (`crates/rs_cam_core/src/reach.rs:5-7`
states the 3.0 figure). The query rectangle is `2r` on a side against 0.696 mm
index cells:

| tool | `radius()` | index cells touched | candidate triangles (≈8/cell) |
|---|---|---|---|
| Ø0.05 probe (the anchor) | 0.025 | 1 | ~8 |
| Ø4 ball (R2.0) | 2.0 | ~(5.75+1)² ≈ 46 | ~365 |
| Ø1-tip taper on Ø6 shank | 3.0 | ~(8.63+1)² ≈ 93 | ~740 |

`drop_cutter_can_contact` (`dropcutter.rs:36`) rejects most of those before
the virtual dispatch, so the effective multiplier is well under the raw
candidate ratio — but it is **not 1×**, and I have no measurement of the
prune rate, so I will not invent one.

**Honest estimate, wide bars.** Three rate bases from §1.2, applied to the
mesh-derived cell counts above. Every row is **per tool, per map**:

| basis | 0.6 mm (118 k) | **0.3 mm (473 k)** | 0.15 mm (1.89 M) |
|---|---:|---:|---:|
| optimistic — anchor B rate (215 k-tri fixture, 241 k cells/s) | 0.5 s | **2.0 s** | 7.8 s |
| **central — anchor A rate (wanaka-native, ~15 k cells/s), taking the G3 gain and the real-tool penalty as cancelling (§1.2)** | **8 s** | **31 s** | **125 s** |
| pessimistic — anchor A with a further 2–3× real-tool penalty on top | 16 – 24 s | 62 – 94 s | 250 – 375 s |

**Verdict on cost.**

* At **0.6 mm** a per-tool map is single-digit to low-tens of seconds. Five
  candidate tools ≈ 40 s. **Affordable at planning time today, uncached.**
* At **0.3 mm** it is ~30 s per tool centrally (2–95 s across the band). Five
  tools ≈ 2.5 min. **Affordable once, per project — not per operation, and not
  interactively without the G4 cache.**
* At **0.15 mm** it is ~2 min per tool centrally. Five tools ≈ 10 min, i.e.
  **1.6× the entire current wanaka `generate_all`** (394 s). **Not affordable
  as a naive per-tool sweep.** Either cache it (G4), share the grid walk
  across tools (G1), or raster it (§5.2).

**Memory is a real second constraint at 0.15 mm, and it is measured.** A
`FinishSurface` carries `SurfaceHeightmap` (8 B `z` + 1 B `covered`) plus
`SlopeMap` (24 B normal + 8 B angle + 8 B curvature) ≈ **49 B/cell**
(`planning/review_2026-07-29/CHECKPOINT_B_EVIDENCE.md:370-378`; the same
document's scaling warning, `:379-386`, puts a `cusp/4` grid on a 300 × 200 mm
part at ~3.84 M cells ≈ **188 MB for the surface alone**). For wanaka:

| spacing | cells | bytes/tool | 5 tools |
|---|---:|---:|---:|
| 0.6 mm | 118 k | 5.8 MB | 29 MB |
| 0.3 mm | 473 k | 23 MB | 116 MB |
| 0.15 mm | 1.89 M | **93 MB** | **463 MB** |

For scale, this board already OOMs simulation at 0.1 mm
(`planning/airrun_2026-08-19/EFFICIENCY_CAMPAIGN_2026-08-23.md:27`). A
residual map does not need the `SlopeMap` half, so a map-only structure is
9 B/cell (17 MB at 0.15 mm for 1 tool) — but if the tier decision wants slope
(and §3.4 says it does), the 49 B/cell figure is the one that applies.

Two independent sanity checks that the range is not fantasy:

One independent sanity check that the central row is not fantasy: the
**rest-field detector already does two of these drops per cell** at
`rest_field.rs:644` and `:653`, on a grid whose default `cell_mm` is 0.5
(`rest_field.rs:151`) and which the wanaka P2 project dials to **0.2 mm**
(`planning/airrun_2026-08-19/wanaka200_p2.toml:928` `rest_cell_mm = 0.2`).
With the margin rule at `rest_field.rs:630-635` that is a 1033² grid ≈
**1.07 M cells × 2 real-tool drops ≈ 2.1 M drops**, and it runs today inside
pencil generation and inside `UnifiedFinish`-with-claims, inside a 394 s
whole-project `generate_all` in which `point_drop_cutter` is 2.85 % of CPU
(`BASELINES.md:1198-1210`). That is *directionally* consistent with the
central row and hard to reconcile with the pessimistic one — the pessimistic
rate would put that single existing rest field at several minutes on its own,
which a 394 s whole-project run with a 2.85 % drop-cutter share does not leave
room for. It is not a controlled comparison and should not be quoted as one.

### 1.4 Is any of this cached today? No — but the blocker named in 2026-08 is now retired

`crates/rs_cam_core/src/geom_cache.rs` memoises exactly three things
(`:6-11`): `build_auto` index, default silhouette, setup-transformed mesh.
**No heightmap, no drop-cutter grid, no classification grid is cached**, and
the cache is not keyed by tool (`cached_auto_index` at `:279`,
`cached_silhouette` at `:302`, `cached_transform` at `:329`).

`CLASSIFICATION_PERF_STUDY.md:379-390` deferred "candidate 4 — cached
classification fields" with a stated precondition: *"`TriangleMesh` has **no
revision or identity field**, so there is nothing to key a cache on today."*
**That precondition has since been met.** `geom_cache.rs:19-53` solves the
identity problem soundly (a `Weak<TriangleMesh>` + `Arc::ptr_eq`, explicitly
argued against the ABA hazard, with the immutability argument at `:43-53`).
A per-`(mesh, tool, cell_size)` surface-map cache can now be keyed the same
way. The study's other two preconditions — a retention bound and invalidation
sentries — are also already demonstrated in that module (`:75-89`, and
`tests/geometry_cache_g8.rs` per `:51-53`).

This is the single cheapest lever on the cost question and it is *already
architecturally paid for*.

---

## 2. `reach.rs` — what it actually computes, and can it run for a candidate tool?

### 2.1 What it computes

`crates/rs_cam_core/src/reach.rs` is **not** a residual map and **not**
`tip_float`. It is a **1-D lateral-reach solve at a single point on a rest
centreline**: given a local valley cross-section, how far sideways from the
apex can this cutter stand while still holding the local depth.

* Input `LocalValley` (`reach.rs:236-243`): `rest_depth_mm` + two
  `ValleySide` (`:156-164`), each a rim distance and a wall rise.
* Output `Reach` (`:274-285`): `left_mm`, `right_mm`, `refused`, `model`.
* The V-model solve: `solve_reach` (`:353-373`), built on `profile_rise`
  (`:321-348`) which scans `width_at_height(u) + (δ−u)·cot θ` and adds a
  one-sided conservatism bound (`:347`).
* The generalisation: `solve_reach_sampled` (`:524-599`) erodes the cutter
  against a `SampledCrossSection` (`:389-393`) with no wall angle at all.
* **`refused == true` IS the tip-float case** — the cutter wedges between
  both walls and cannot hold the depth on the centreline (`:280-282`,
  `:370`). The reported finding type is
  `compute::config::TipFloatFinding` (named at `reach.rs:52`).
* `suggested_offset_stepover_mm` (`:707-709`) = `working_half_width_mm ×
  0.5` (`:660-664`, `:669`), i.e. **half the band one pass works at this
  depth**, floored at the cusp radius. `coverage_cap_passes` (`:742-752`)
  and `route` (`:757-766`) turn that into the pencil-vs-clearing verdict.

### 2.2 Domain — this is the important limitation

Every entry point is **per-point on a measured centreline**, not per-cell on a
grid. Production evaluates it inside `rest_field::detect_rest_valleys`, once
per sample of each traced ridge polyline:
`crates/rs_cam_core/src/rest_field.rs:922-948` (the `PRODUCTION_REACH_MODEL`
dispatch at `:932-937`), feeding `CenterlineSample { valley, reach }`
(`:355-357`). The cross-section itself is measured by `measure_cross_section`
(`:510-608`) — two outward walks from the ridge cell over the rest grid.

So `reach.rs` answers **"can this tool work the band around *this crease*?"**
It does **not** answer "does an R2 ball fit at this arbitrary (x,y)". Turning
it into a map would require a cross-section at every cell, i.e. a
perpendicular direction at every cell — which only exists where a ridge does.

### 2.3 Can it be evaluated for a candidate tool that is not the op's tool? **Yes, trivially.**

Every public function in the module takes `cutter: &dyn MillingCutter` as a
plain parameter and holds no state:

* `profile_rise(cutter, wall_run, depth_mm)` — `reach.rs:321`
* `solve_reach(cutter, valley)` — `reach.rs:353`
* `solve_reach_sampled(cutter, section, rest_depth_mm)` — `reach.rs:524`
* `working_half_width_mm(cutter, depth_mm)` — `reach.rs:660`
* `suggested_offset_stepover_mm(cutter, depth_mm)` — `reach.rs:707`
* `coverage_cap_passes(cutter, depth_mm, stepover, passes)` — `reach.rs:742`

The module is pure and dependency-free apart from `crate::tool::MillingCutter`
(`reach.rs:128`). Its own test suite already evaluates **three different
tools against one fixed valley** (`reach.rs:860-879`,
`:950-965`, `:1000-1009`), and the Checkpoint A matrix scores 176 valley
cells × 3 tools (`reach.rs:8-9`).

**Concretely: for a candidate tool T you can already ask, at every sample of
every detected centreline, `solve_reach(T, &sample.valley)` and get back "T
reaches ±X here" or "T is refused here" — for free, from a
`RestCenterline` that was measured with a different tool.** The measurement
(`CenterlineSample.valley`, `rest_field.rs:355`) is tool-independent
geometry; only the `reach` field is tool-specific.

That is a genuinely reusable per-tool reachability oracle. It is *sparse*
(centrelines only), not a map.

### 2.4 What `reach.rs` says about resolution — read this before choosing a cell size

`PRODUCTION_REACH_MODEL` is pinned to `WallAngleV`, not the strictly-better
sampled model, and the constant's doc says why (`reach.rs:601-649`):
the sampled model's accuracy is **pitch-bound**, and at the shipped 0.5 mm
rest cell it recovers only ~50 % of reachable detail with 10–14 routing
over-claims, needing **0.01–0.002 mm pitch** for 100 % — *"50× to 250× finer
than the shipped cell, i.e. 2,500× to 62,500× the cells"* (`:635-636`).

**Implication for a tier map:** if the tier decision is ever expressed as a
reach solve on a sampled cross-section, the grid resolution question is not
free — the honest failure mode at a coarse pitch is *under-claimed reach*
(a MISS, i.e. detail handed to a finer tool than needed), never a gouge
(`reach.rs:509-513`). That is the safe direction for a tier map, but it will
systematically over-assign work to the fine tools.

---

## 3. How rest analysis expresses "what tool X left behind"

*(Detailed structural map produced by a parallel sweep; consolidated here.)*

### 3.1 The two unrelated subsystems — do not confuse them

* **2D polygon rest** — `crates/rs_cam_core/src/rest.rs`. Offsets a polygon
  inward by a scalar `prev_tool_radius` (`rest.rs:36-53`, `:102`) and
  scan-lines the leftover ring; refuses when
  `tool_radius >= prev_tool_radius` (`rest.rs:97-99`). **A radius scalar,
  never the prior tool's profile.** Irrelevant to 3D terrain.
* **3D rest-depth field** — `crates/rs_cam_core/src/rest_field.rs`. This is
  the engine. Zero code overlap, stated at `rest_field.rs:37-40`.

### 3.2 The core equation and the threshold site

Module doc, `crates/rs_cam_core/src/rest_field.rs:9-11`:

```text
rest(x, y) = drop_z(reference_tool, x, y) − drop_z(pencil_tool, x, y)
```

Three arms, all in one sample closure at `rest_field.rs:640-683`:

| arm | line | expression |
|---|---|---|
| `Cutter { is_surface_probe: false }` | `:653-665` | `rc.z − pc.z` — bigger reference floats higher |
| `Cutter { is_surface_probe: true }` | `:663` | `pc.z − rc.z` — how far the fine tool floats above a Ø0.1 bare-surface probe |
| `Stock(&TriDexelStock)` | `:668-680` | `stock_top_z(x,y) − pc.z` — **actual machined material**, nearest-cell, explicitly no interpolation (`:671-672`) |

**The `residual > threshold` comparison lives at exactly one place:**

```rust
// crates/rs_cam_core/src/rest_field.rs:700
let threshold = params.min_valley_depth.max(0.0);
// :715
let erode_cells = (pencil.radius().max(reference.erosion_radius()) / cell).ceil();
// :717-723  ← THE island predicate
.map(|((&ct, &dt), &rv)| ct && dt >= erode_cells && rv > threshold)
```

`erosion_radius()` (`:78-83`) is `tool.radius()` for a cutter, `0.0` for
stock — it kills the false-high "moat" a reference tool reads when it hangs
off the part edge.

A **second, independent** application of the same field with a *different*
dial is the UnifiedFinish S4 keep-mask,
`crates/rs_cam_core/src/unified_finish.rs:1592-1596`:
`r.is_nan() || f64::from(r) >= cfg.min_rest_depth_mm` — note `>=` here vs
`>` at `rest_field.rs:722`, and NaN keeps coverage.

### 3.3 The configs

`RestAnalysisConfig` — `crates/rs_cam_core/src/compute/config.rs:1519-1559`,
defaults at `:1561-1575`:

| field | line | default |
|---|---|---|
| `enabled` | `config.rs:1521` | `false` |
| `reference_tool_id: Option<ToolId>` | `config.rs:1526` | `None` |
| `cell_mm` | `config.rs:1528` | `0.5` |
| `min_valley_depth` | `config.rs:1531` | `0.05` |
| `region_margin_mm` | `config.rs:1535` | `0.5` |
| `offset_stepover_mm: Option<f64>` | `config.rs:1551` | `None` (→ ask `reach`) |
| `num_offset_passes: Option<usize>` | `config.rs:1558` | `None` |

Its `min_valley_depth` doc (`config.rs:1529-1531`) is verbatim the thing this
investigation wants: *"a cell counts as REST material once the reference
floats more than this above the true surface."*

Lives on `ToolpathConfig.rest_analysis`
(`crates/rs_cam_core/src/session/mod.rs:683`), round-trips through
`crates/rs_cam_core/src/session/project_file.rs:458-463`, has a GUI panel
(`crates/rs_cam_viz/src/ui/properties/mod.rs:4310-4403`) and an MCP setter
(`crates/rs_cam_viz/src/mcp_server.rs:1108-1130`).

Claims machinery (UnifiedFinish-internal):

| thing | where |
|---|---|
| `ClaimsReference` (`Auto`/`SelfProbe`/`MachinedStock`) | `crates/rs_cam_core/src/unified_finish.rs:314-348` |
| `CreaseReference` (2-valued, what the detector switches on) | `unified_finish.rs:276-293` |
| `ClaimsReferenceResolution` — the ONE mapping site | `unified_finish.rs:363-401`, `.reference()` at `:405-415` |
| `pencil_claims` (master gate) | `crates/rs_cam_core/src/compute/operation_configs.rs:1005`; gate at `crates/rs_cam_core/src/compute/execute.rs:2144` |
| `min_rest_depth_mm` | `operation_configs.rs:1016` (default `0.02`); consumed at `unified_finish.rs:1595`; also **floors** `min_valley_depth` at `execute.rs:2219` |
| `claims_reference` | `operation_configs.rs:1036`, default `Auto` at `:1158-1160` |
| `territory_stock: Option<&TriDexelStock>` | `unified_finish.rs:546`; produced at `execute.rs:2151-2157` (XY-bbox overlap guarded) |
| the run | `unified_finish.rs:1407-1550`, detector call at `:1454` |

Wanaka P2 today has `pencil_claims = false`, `min_rest_depth_mm = 0.02`,
`claims_reference = "auto"`
(`planning/airrun_2026-08-19/wanaka200_p2.toml:830-832`), with the pencil op
at `min_valley_depth = 0.1`, `rest_cell_mm = 0.2` (`:922`, `:928`).

### 3.4 ⚠ The slope bias — the biggest technical risk in the whole idea

The drop-cutter Z is the **tool-centre (CL) offset surface**, not the machined
surface. `CLASSIFICATION_PERF_STUDY.md:74-86` gives the law and the numbers:
for a facet of unit normal `n`, the CL sits at
`z_plane + R·(1 − n.z)/n.z` above the true surface, i.e. `R·(sec θ − 1)`:

| facet slope | offset for R = 0.025 (measured, `:80-85`) |
|---|---|
| 0° | 0 µm |
| 45° | 10 µm |
| 60° | 25 µm |
| 80° | 119 µm |

The offset is **linear in R**. So the tool-vs-tool residual on a *plain
sloped plane* — which any ball machines perfectly — is

```text
rest = (R_ref − R_fine) · (sec θ − 1)
```

For R2.0 vs R0.5 that is **0.62 mm at 45°** and **1.5 mm at 60°** — one to two
orders of magnitude above `min_valley_depth`'s 0.05 mm default. **A naive
tool-pair residual map on terrain will mark every slope as residual.**

The repo knows this, but only at the **ridge** stage, not the **mask** stage.
`rest_field.rs:1369-1382` and `:1384-1404`: *"A ball resting on a
constant-slope plane floats a position-independent height, so plane-wall
V-grooves (and any uniform slope) read a CONSTANT rest plateau"* — handled by
requiring **prominence** in the non-max-suppression
(`NMS_PROMINENCE_FRACTION = 0.1` at `:1378`, floor `0.005 mm` at `:1382`), so
plateaus yield no ridge candidates. The **mask** at `rest_field.rs:722` has no
such protection, and the mask is what
`region_polygons_from_mask` consumes (`:830-836`) and therefore what
`DerivedRestRegions` confines a downstream op to
(`rest_field.rs:33-35` says so in as many words: *"the mask is still used
(untouched) for `region_polygons_from_mask` / the P2 selective-finishing
boundary source"*).

**Consequence for T1:** the island mask that today drives selective finishing
is *ridge-clean but plateau-dirty*. A tier map built straight off it will hand
the fine tool every slope on the board — exactly the outcome the multi-tool
project is trying to avoid. Three ways out, in increasing order of honesty:

1. Subtract the analytic plateau term `(R_ref − R_fine)·(sec θ − 1)` per cell
   using the slope map the classification grid already carries
   (`FinishSurface.slope_map`, `finish_setup.rs:302`). Cheap, closed-form, and
   it degenerates correctly to zero on flats. **Untested; nothing in the repo
   does this today.**
2. Use `RestReference::Stock` instead of a nominal tool
   (`rest_field.rs:64-70`): *"Strictly better than any nominal tool drop — it
   bakes in the prior TOOLPATH pattern (scallop cusps, skipped boundaries,
   walls the finish never visited), not just the prior tool's shape."* This is
   the correct answer and it is already shipped — but it requires the coarse
   tier's toolpath to have been **generated and simulated** first, i.e. it is
   a cascade, not a plan-time oracle.
3. Compute a true achievable surface by morphologically closing the surface
   with the tool (dilate-then-erode in 3-D). **Nothing in the repo does this.**
   The only `morphological_close` present is 2-D and operates on a *binary
   mask*, not a height field
   (`crates/rs_cam_core/src/finish_planner.rs:740-750`).

### 3.5 Region / island representation

Both a bitmask and polygon rings, with one converter.

* Masks are `Grid2<bool>` throughout `rest_field.rs` (built `:727`, cleaned
  `:816-824`); `Grid2` itself is `crates/rs_cam_core/src/grid2.rs:70-74`,
  row-major `r*nx + c` (`:33`), which the module doc says is the convention
  shared by `RestGrid`, `SurfaceHeightmap`, `SlopeMap`, `DropCutterGrid` and
  `MaterialGrid` (`grid2.rs:5-9`).
* The converter: `region_polygons_from_mask` —
  `crates/rs_cam_core/src/region_mask.rs:49-57`, real body at `:88-207`. EDT
  dilation (`:103-111`), optional coverage clamp (`:118-140`), **marching
  squares** (`:149` → `crates/rs_cam_core/src/contour_extract.rs:241-255`),
  degenerate-loop filter (`:151-156`), containment + winding (`:158-161`),
  area-sorted with a **`MAX_REST_REGIONS = 64` cap** (`region_mask.rs:27`,
  `:182-204`).
* Downstream the polygons become `RegionSet<'a>`
  (`crates/rs_cam_core/src/region_set.rs:29-31`), disjoint by construction
  *"because they come from a marching-squares rest-region detector"* (`:1-16`),
  with `contains` (`:71-73`) and `processed(keep_outs, offset)` (`:92-109`).
* Pathology guard: `classify_rest_regions`
  (`rest_field.rs:1123-1146`) → `TooManyIslands` / `SingleGiantRegion`, with
  the denominator that **must** be `RestGrid::covered_footprint_area_mm2()`
  (`:266-268`), not a bbox.
* Transport: `AnnotatedToolpath::rest_grid` / `rest_regions`
  (`crates/rs_cam_core/src/toolpath_spans.rs:455,465`), attached by pencil at
  `execute.rs:2008-2011` and by UnifiedFinish claims at `execute.rs:2337-2338`.

### 3.6 A different tool is ALREADY evaluated — three production call sites

`detect_rest_valleys(mesh, index, pencil, reference, params)` —
`rest_field.rs:616-622` — takes the op's own cutter **and** an independent
`RestReference` (`:56-71`).

1. **Standalone Pencil**: `crates/rs_cam_core/src/pencil.rs:1681-1769`.
   Priority: `Stock` (`:1693-1714`, chosen `:1724-1725`) → a **real library
   reference tool** from `params.reference_cutter` (`pencil.rs:174`, resolved
   `:1582-1583`, emitted `:1728-1735`; populated from `ctx.reference_tool_cfg`
   at `execute.rs:1950-1953`) → a **synthesised nominal ball**
   (`pencil.rs:1585-1590`, `:1736-1743`) → self-probe (`:1744-1751`).
2. **Generic, any op family**:
   `attach_generic_rest_analysis(generated, mesh, index, tool_def,
   reference_tool_cfg, initial_stock, cfg, findings)` —
   `crates/rs_cam_core/src/compute/execute.rs:3023-3091`, gated at
   `:2988-3004`, reference chain `resolve_rest_reference` at `:3104-3127`
   (stock → configured reference tool → Ø0.1 probe), detector call at `:3088`.
   Its doc (`:3009-3019`) says it plainly: *"runs the same rest-depth detector
   … with THIS toolpath's own tool as the fine cutter."*
3. **UnifiedFinish claims**: `unified_finish.rs:1432-1454`. Self-probe or
   `Stock` **only** — there is no `reference_tool_id` on
   `UnifiedFinishConfig`, because the op is single-tool by construction
   (`unified_finish.rs:550-553`).

Two asymmetries to know:

* **Stock outranks a configured reference tool unconditionally** in both
  non-claims paths (`pencil.rs:1724-1725`, `execute.rs:3110-3121`). An
  operator-set `reference_tool_id` is silently unused on any op cutting
  `FromRemainingStock` with a simulated snapshot in scope.
* **`erosion_radius()` uses `tool.radius()` = the ENVELOPE**
  (`rest_field.rs:78-83`), which on a tapered ball is the *shank*. On the
  shipped Ø1-tip/Ø6-shank taper that erodes the trust region by 3.0 mm, not
  0.5 — a 3 mm dead band around the part edge and around every hole.

### 3.7 The shipped two-tier loop, end to end

```text
op A (coarse tool) generates
  → rest_analysis.enabled  →  attach_generic_rest_analysis
      → detect_rest_valleys(mesh, index, toolA, reference=probe|refTool|Stock)
      → rest > min_valley_depth  →  8-connected components  →  region_polygons
  → AnnotatedToolpath.rest_regions           (toolpath_spans.rs:465)
op B (fine tool), BoundarySource::DerivedRestRegions{ source_toolpath_id: A }
  → session::resolve_derived_rest_region_polys                (session/compute.rs:1794-1843)
  → RegionSet passed as `boundary_regions`   (execute.rs:2864, 2892)
  → mesh-finish family pre-clips generation to those islands  (execute.rs:2824-2831)
```

`BoundarySource::DerivedRestRegions` is at
`crates/rs_cam_core/src/compute/config.rs:1454-1456`; the resolver reads
`result.annotated().rest_regions` at `session/compute.rs:1834` and refuses
self-reference at `:1811-1818`.

**That is the two-tier version, already shipped and operator-configurable.
The n-tier version is a chain of these.**

---

## 4. The July "P2 selective finishing" work — `2195d33`

`6a7e16482a754d87633b5f95ca681b0e6fadb5a4`, **2026-07-07**,
*"feat(finishing): P2 selective finishing — rest regions to derived boundaries
to pre-clipped fine ops"*. 40 files. Ledger:
`planning/finishing_stack_review_2026-07.md`.

### 4.1 What "selective" means there

**Selective = a fine-tool finish op is confined to the rest-region islands a
previous op's rest-depth analysis found, instead of running all over the
model.** Nothing more exotic. The selection criterion is exactly the mask of
§3.2 — `rest > min_valley_depth` on a two-tool drop-cutter difference — and
the unit of selection is a `Polygon2` island.

**Its own live-validation result is the headline number for this whole
investigation**, quoted verbatim from the commit message:

> live-validated on wanaka: selective fine scallop cuts **1593 mm vs 7788 mm**
> all-over baseline, **~80 % less**, confined to rest-region islands

That is the two-tier version of the operator's ask, already measured on this
board, in July.

### 4.2 The three layers it added, and their state on master today

| layer | what it added | on master |
|---|---|---|
| **P2.1** — regions | EDT-dilated rest mask → marching squares → containment-grouped `Polygon2`s, published on `RestFieldResult.region_polygons` and `AnnotatedToolpath.rest_regions` | **alive**: `crates/rs_cam_core/src/region_mask.rs:88-207`; `rest_field.rs:830-836`, `:381`; `crates/rs_cam_core/src/toolpath_spans.rs:455,465` |
| **P2.2** — boundary source | `BoundarySource::DerivedRestRegions { source_toolpath_id }` with fail-hard staleness preconditions; `clip_toolpath_to_boundary_set_with_provenance` as the **sole** clip walk (the single-polygon fn delegates to it); `apply_boundary_clip_multi`; GUI picker, MCP set/get, project-IO round-trip | **alive**: `crates/rs_cam_core/src/compute/config.rs:1454-1456`; `crates/rs_cam_core/src/boundary.rs:275-291` (single-polygon delegate at `:249-255`); `crates/rs_cam_core/src/session/compute.rs:2127` and the resolver at `:1794-1843` |
| **P2.3** — pre-clip | `ExecutionContext.boundary_regions` threaded into **all 7 mesh-finish ops** (predicate ahead of the drop-cutter query where cheap, else at the `point_runs` splitter; waterline clips closed contours) | **alive**: `crates/rs_cam_core/src/compute/execute.rs:2864`, `:2892`, doc at `:2824-2831`; 73 `boundary_regions` references across `scallop.rs`, `waterline.rs`, `radial_finish.rs`, `spiral_finish.rs`, `ramp_finish.rs`, `horizontal_finish.rs`, `steep_shallow.rs` |

Also landed and still present: the rest heatmap overlay
(`crates/rs_cam_core/src/rest_heatmap_mesh.rs`), `Polygon2` boolean-ops
hardening (`crates/rs_cam_core/src/polygon.rs`), and the GUI-worker fix that
had been *silently dropping `rest_grid` / `rest_regions` / `planner_engagement`
from every GUI result*.

### 4.3 Is the region-selection machinery reusable for tier maps? **Yes, and it is the best-tested part of the chain.**

* **Input**: `&[Polygon2]` — a *set of disjoint islands*, not a single polygon.
  The set semantics are the load-bearing design decision, and
  `boundary.rs:257-274` states it explicitly: *"A point is considered inside
  the boundary if it is inside ANY polygon… merging them into one
  polygon-with-holes first would be wrong (there's no shared exterior), but
  'inside island A OR inside island B' is exactly what 'the tool may cut here'
  means for that source."*
* **Output**: a clipped `Toolpath` plus a move-index `mapping`
  (`boundary.rs:240-245`), so spans and annotations survive the clip.
* **Single walk**: `clip_toolpath_to_boundary_set_with_provenance` is the sole
  implementation; the single-polygon entry point delegates
  (`boundary.rs:271-274`), so the two cannot disagree.
* **Pre-clip, not post-clip**: the regions reach generation as
  `boundary_regions` and the finish ops skip work *before* the drop-cutter
  query where they can (`execute.rs:2824-2831`). That is what turned 7788 mm
  into 1593 mm — the fine tool never plans the excluded ground.

**For an n-tier map, tier *k*'s territory is a `Vec<Polygon2>` and every one of
these layers takes it unchanged.** What P2 does *not* provide is the thing that
decides *which* tier owns a region: today the answer is always "the fine tool
owns the rest islands of exactly one named upstream toolpath"
(`BoundarySource::DerivedRestRegions` carries a single `source_toolpath_id`,
`config.rs:1455`). An n-tier planner would produce n disjoint region sets from
one residual sweep rather than chaining n pairwise analyses — but it would feed
them into precisely this machinery.

---

## 5. Reuse verdict

### 5.1 The right seam

**`rest_field::detect_rest_valleys` + `attach_generic_rest_analysis` is the
seam.** Not `reach.rs`, not `finish_planner::decompose`, not a new
drop-cutter path.

Reasons:

* It is the only thing in the tree that already computes a **per-cell,
  tool-parameterised residual** and turns it into **islands as polygons**
  (`rest_field.rs:640-683` → `:722` → `:830-836`).
* It is already **op-agnostic** (`execute.rs:3023-3091`) and already
  **operator-configurable** (`RestAnalysisConfig`, `config.rs:1519-1559`),
  with GUI and MCP surfaces.
* Its output already **feeds a downstream operation's machining boundary**
  (`BoundarySource::DerivedRestRegions`, `config.rs:1454`), which is exactly
  "this tool only cuts here".
* Its reference is already a `&dyn MillingCutter` you can hand any candidate
  tool (`rest_field.rs:60-63`).

**Do not build a new full-grid drop-cutter map.** The one you would build is
`SurfaceHeightmap::from_mesh_with_cancel` (`slope.rs:141-234`), reachable
through `build_finish_surface_with_policy_and_cancel`
(`finish_setup.rs:354-385`), which takes any cutter and already tags its
output `SurfaceSampler::CutterOffset` (`:383`) — but a *difference of two
CutterOffset surfaces is precisely what `detect_rest_valleys` computes*, on
one grid walk, with the boundary erosion and the component extraction already
correct.

The complementary piece worth adopting: **`sample_classification_grid` at
`ClassificationSampler::TileRaster`** (`classify_probe.rs:248-264`, `:136`)
gives the **true** surface at 187–198× the speed of a drop-cutter sweep
(`CLASSIFICATION_PERF_STUDY.md:698`). If the residual is ever defined against
*truth* rather than against another tool, that is the free half of the
subtraction, and it removes the need for the Ø0.1 self-probe drop entirely.

**But TileRaster cannot produce a per-*tool* map, and this must not be
assumed away.** It rasterises triangle planes and keeps the per-cell max
(`CLASSIFICATION_PERF_STUDY.md:122-133` candidate 1, `:147-155` the tiled form
of it, and §5.3's measured proof that the arms *are* the surface at `:310-313`)
— it has no tool in it at all. A CL offset surface is a Minkowski dilation of the surface by the
tool, which needs a stamp with radial extent, not a per-cell plane evaluation.
So a per-tool map necessarily runs the drop-cutter family.

That said, the 187× result **is** the existence proof that a raster-shaped
per-tool CL sampler is possible: splat each triangle's tool-offset surface into
every cell within the tool radius and keep the max — which is structurally the
same operation `dexel_stock`'s cutter stamping already performs. Nobody has
built it; it is the obvious escape hatch if the 0.15 mm row of §5.3 turns out
to be required.

### 5.2 What is missing

| # | gap | where it would go | size |
|---|---|---|---|
| G1 | **n-tool residual in one pass.** `detect_rest_valleys` computes exactly one `(fine, reference)` pair. An n-tier map wants n drops per cell in one grid walk — and, since the largest tool's index query is a superset of every smaller tool's, **one `index.query` at max radius could serve all n**, with `drop_cutter_can_contact` (`dropcutter.rs:36`) filtering per tool. That is strictly cheaper than n separate 2-drop passes. | new fn beside `detect_rest_valleys`, `rest_field.rs` | medium |
| G2 | **The slope-plateau bias (§3.4).** The mask has no plateau protection; only the ridge NMS does (`rest_field.rs:1369-1382`). Untreated, every slope reads as residual. | either a slope-compensated threshold in the mask predicate (`rest_field.rs:722`) or a mandate to use `RestReference::Stock` | **the real work** |
| G3 | **A tier planner.** Nothing chooses "R2 here, R0.5 there". `finish_planner::decompose` (`finish_planner.rs:352-357`) partitions by **slope band** off a `SlopeMap`, not by residual. But its pipeline is band-agnostic below step 1: hysteresis mask (`:410-425`) → `morphological_close` (`:433-441`, fn at `:740`) → 3-way label grid (`:443-459`) → `absorb_small_regions` (`:461-463`, fn at `:761`) → `region_polygons_from_mask_clamped` per band (`:480-511`). **Steps 2, 4 and 6 operate on `Vec<bool>` / `Vec<Option<FinishBand>>` and would work verbatim over tier labels** — only step 1's slope thresholds and the 3-valued `FinishBand` (`:97`) are hard-wired, and the helpers are private (`fn`, `:712/:740/:761/:941`) so they need `pub(crate)` or a new entry point. | new entry point reusing steps 2/4/6 over a residual mask | medium |
| G4 | **No per-tool surface cache.** `geom_cache.rs:6-11` caches index/silhouette/transform only, and nothing is keyed by tool. The 2026-08 blocker ("no mesh identity to key on") is retired by `geom_cache.rs:19-53`. Both prior reviews asked for exactly this: *"Cache classification fields by mesh, tolerance, and requested resolution"* and *"Reuse one high-resolution true-surface field across planner consumers"* (`planning/review_2026-07-29/RADIUS_AUDIT.md:83`, opportunities 3 and 4). Note the retention bound is real — 49 B/cell (§1.3). | extend `geom_cache` with a `(mesh, tool, cell)` surface slot | small, high value |
| G5 | **"Strictly faster at equal cusp height" is unmodelled.** Nothing in the tree compares two tools' *time* at a fixed cusp. Every ingredient is present — `stepover_from_scallop_flat(tool_radius, scallop_height)` (`crates/rs_cam_core/src/scallop_math.rs:30-35`) and the **curvature-aware** `stepover_from_scallop_curved(tool_radius, scallop_height, curvature)` (`:75-82`), which is the correct form on terrain since the classification `SlopeMap` already carries curvature; plus link costing via `machine_kinematics::LinkKinematics` (`crates/rs_cam_core/src/machine_kinematics.rs:328`). What is absent is the comparator that says *at cusp h, tool A covers this island in T_A and tool B in T_B*. | new, on top of `scallop_math` | medium, and it is what makes the tier choice *economic* rather than *geometric* |
| G6 | **`erosion_radius()` reads the envelope** (`rest_field.rs:78-83`), so a tapered tool blanks a 3 mm ring. On a 240 × 250 board that is ~4 % of the area, and it is the *edge*, where the operator cares. | `rest_field.rs:715` | small, ledgerable to the radius programme |

### 5.3 Honest cost estimates

All figures **per tool, per map**, wanaka mesh (200 × 200 mm, 661 k tri),
central estimate with the band from §1.3 in brackets. **No committed benchmark
measures this exact operation** — see §1.2 for the four anchors and why they
disagree.

| item | estimate | basis |
|---|---|---|
| full-grid per-tool drop-cutter map, **0.6 mm** (344² = 118 k cells) | **~8 s** [0.5 – 24] | §1.3 |
| same at **0.3 mm** (688² = 473 k) | **~31 s** [2 – 94] | §1.3 |
| same at **0.15 mm** (1374² = 1.89 M) | **~125 s** [8 – 375] | §1.3 |
| the **true-surface** half of a residual (TileRaster) | **~0.01 s** at 473 k cells | 0.016 s at 720 k, `CLASSIFICATION_PERF_STUDY.md:698` — effectively free |
| n-tier map, n tools, one shared grid walk (G1) | **< n ×** the above, plausibly ~0.4–0.6 n by sharing the walk and one max-radius index query | structural; **unmeasured** |
| memory, 5 tiers at 0.3 / 0.15 mm | **116 MB / 463 MB** | 49 B/cell measured, `CHECKPOINT_B_EVIDENCE.md:370-378`; the board already OOMs sim at 0.1 mm (`EFFICIENCY_CAMPAIGN_2026-08-23.md:27`) |
| what this is spent against | a whole wanaka200 `generate_all` fixpoint (8 ops) is **394 s** (`BASELINES.md:610-615`), of which `point_drop_cutter` is **2.85 % of CPU** (`:1198-1210`) | — |

**Bottom line on cost.** At **0.6 mm** a five-tool sweep is ~40 s and needs
nothing new. At **0.3 mm** it is ~2.5 min and 116 MB — fine once per project,
not per operation, and it wants G4. At **0.15 mm** a naive five-tool sweep is
~10 min and 463 MB, i.e. **1.6× the entire current `generate_all` and a memory
figure this board is already near the edge of** — that resolution needs G1
(shared walk) or G4 (cache) or a rasterised CL surface (§5.2), not a naive
loop.

**But cost is still not the blocker. The slope bias (G2) is** — it is a
correctness problem, and no amount of grid budget fixes it.

### 5.4 Suggested order of attack (T1's opinion, for the other tracks to argue with)

1. **Reproduce `2195d33`'s 1593-vs-7788 result on today's master** before
   building anything. It is the only end-to-end evidence that exists, it is on
   this board, and it is a year-quarter old. If it still holds, the two-tier
   case is closed and the question is only *how many tiers*.
2. **Measure the slope bias directly** (G2): dump `RestGrid.rest` for an
   R2-vs-R0.5 pair on wanaka and render it against the slope map. Either the
   analytic `(R_ref − R_fine)(sec θ − 1)` term dominates the map, in which case
   G2 is the whole project, or it does not, in which case say why.
3. **Then** decide between the analytic slope compensation and the
   `RestReference::Stock` cascade (§3.4 options 1 vs 2). They have very
   different pipeline shapes: one is plan-time, the other is
   generate-simulate-generate.
4. Only after that does the n-tier pass (G1), the tier planner (G3) and the
   cost work (G4) become worth building.

### 5.5 Three things not to do

1. **Do not use `reach.rs` as the map primitive.** It is per-point on a
   centreline, and `PRODUCTION_REACH_MODEL` is pinned to the V model
   *because* the sampled model needs a 50–250× finer grid
   (`reach.rs:626-637`). Use it as the *sparse* per-tool oracle it already is
   (§2.3) — for scoring a candidate tool against creases another tool found.
2. **Do not tidy the drop-cutter allocations expecting a win.** Pre-registered
   and refuted at 0.99–1.14× (`CLASSIFICATION_PERF_STUDY.md:88-113`, `:355`).
3. **Do not read a tool-vs-tool CL difference as machined residual on sloped
   ground.** §3.4. `RestReference::Stock` is the honest reference and the
   repo says so (`rest_field.rs:64-70`).
