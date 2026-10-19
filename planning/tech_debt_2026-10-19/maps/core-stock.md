# Map: core-stock

Scope: `crates/rs_cam_core/src/{stock,dexel_stock,metrology,maps}/`. Baseline HEAD 99b6fb41.
Paths below are relative to `crates/rs_cam_core/src/` unless they start with `crates/`.
In-flux check: no file under the four folders appears in `instruments/in_flux_paths.txt`. Callers in `session/` and `compute/` may be in flux.
Counts come from `rg`, include in-area uses unless stated, and are approximate.
Total: about 35,200 lines (stock 13.5k, dexel_stock 11.2k, maps 7.8k, metrology 2.7k). Inline `#[cfg(test)]` modules are 20-35% of the big files.

## 1. Purpose and file map

Purpose. `dexel_stock` is the simulation engine: it stamps a cutter along a toolpath into a tri-dexel stock. `stock` holds the data model beside the engine (dexel grid, material slots, cut trace, triage, collision, meshes). `maps` is a set of grid-walk maps over the model bounding box (multi-tool tier map, reach map) with bounded mesh-keyed caches and heat-map meshes. `metrology` is a set of "rulers" for finished surfaces (spacing, floor, curvature census, union coverage, candidate costing). Metrology has no production caller; only tests use it (section 9).

### stock/ (13.5k)
| file | responsibility | lines |
|---|---|---|
| `stock/mod.rs` | facade, 11 `pub mod`, no re-exports | 17 |
| `stock/dexel.rs` | `DexelSegment/Ray/Axis/Grid`, `LatticeWindow`, `MaterialMask`, ray interval ops (prod to :789) | 1483 |
| `stock/material_slot.rs` | `MaterialSlot(u8)`, `MaterialSlotTable` (cap 8), `SlotVolumes`, `SlotTally`, `MaterialCut` | 422 |
| `stock/cut_as.rs` | `CutAs`, `EffectiveMaterial`, `MaterialPopulations`, `NotJudged`, `POPULATION_EXCLUDED` | 369 |
| `stock/simulation_cut.rs` | cut trace types: sample, issue, hotspot, summaries, `SimulationCutTrace`, `SimulationCutArtifact`, `AirCutRatios` trait | 935 |
| `stock/simulation_cut/accumulate.rs` | `SummaryAccumulator`, `KinematicsAccumulator`, `accumulate_by_span` | 292 |
| `stock/simulation_cut/analysis.rs` | `SimulationCutTrace::from_samples*` assembly | 418 |
| `stock/simulation_cut/reporting.rs` | `rebase_cutting_times`, `publish_cycle_times`, JSON artifact write/prune | 286 |
| `stock/simulation_cut/tests.rs` | tests | 1310 |
| `stock/sim_triage.rs` | `SimulationTriage::build*`, Findings A/B/C/D classes, entry-load, link-cut, standing-material, dedup (prod to :1816) | 2581 |
| `stock/sim_measurability.rs` | `SimMetric`, `Measurability`, `MeasurabilityReport::from_trace` (abstention) | 573 |
| `stock/collision.rs` | `ToolAssembly`, holder/body bands, `HolderCollisionCheck`, `check_collisions*`, rapid-vs-stock scan, `RapidClearanceCheck` (prod to :810) | 1266 |
| `stock/radial_profile.rs` | `RadialProfileLUT` cutter height LUT | 239 |
| `stock/stock_mesh.rs` | `StockMesh`, `DisplayMeshDegrade` | 223 |
| `stock/dexel_mesh.rs` | dexel grid to surface mesh, colour consts `UNCUT_*`/`CUT_*`, drill cylinders (prod to :557) | 1215 |
| `stock/dexel_mesh_mc.rs` (+`dexel_mesh_mc/stride_tests.rs` 659) | Z-grid marching cubes, strided display mesh | 1188 |

### dexel_stock/ (11.2k)
| file | responsibility | lines |
|---|---|---|
| `dexel_stock/mod.rs` | `TriDexelStock` (all fields `pub`), `DrillRemovalReport`, probes (`max_top_z_in_disc`, `clearance_*`), `apply_drill_op`, `checkpoint` (prod to :837) | 1612 |
| `dexel_stock/simulation.rs` | the nine `simulate_toolpath*` variants, chip-thickness stats (prod to :1815) | 2068 |
| `dexel_stock/stamping.rs` | per-cell stamp kernels, `StampTarget` trait, `for_each_covered_cell`, `walk_cell_box`, `FRESH_MATERIAL_THRESHOLD_MM` (prod to :2096) | 3081 |
| `dexel_stock/swept.rs` | swept-volume stamping (lateral moves) | 1351 |
| `dexel_stock/band.rs`, `band_batch.rs`, `tile_mip.rs` | row bands, the one batch dispatcher (`BandJob`, `BandPartial`), coarse max-top mip | 391 / 385 / 374 |
| `dexel_stock/whole_path.rs` | metric-route dispatch (`StampDispatch`) | 434 |
| `dexel_stock/playback.rs` | non-metric replay route (`PlaybackDispatch`) | 385 |
| `dexel_stock/sample_coalesce.rs` | trace sample step (G-SIMMEM) | 437 |
| `dexel_stock/stock_edit.rs` | S3 column edits (outline fill/extrude, mesh cast) | 427 |
| `dexel_stock/cut_direction.rs` | `StockCutDirection` | 176 |

### maps/ (7.8k)
| file | responsibility | lines |
|---|---|---|
| `maps/mod.rs`, `maps/grid.rs` | facade; `GridSpec`, crate-private `walk_rows` (rayon or serial, cancel per row) | 20 / 138 |
| `maps/tier_map.rs` | `compute_tier_map`, `TierLadder`, `TierMapParams`, `TierMapError`, drop counter | 1118 |
| `maps/tier_islands.rs` | `extract_tier_islands`, `TierIslandParams/Error/Set`, cap + band advisory, SVG dump | 1581 |
| `maps/tier_flute_reach.rs` | F3c advisory: owned cells the tool reaches only above its flutes | 177 |
| `maps/reach_map.rs` | `compute_reach_map`, `ReachMap`, params/request, tolerance source, colour ramp, overlay mesh (no inline tests) | 1900 |
| `maps/rest_heatmap_mesh.rs` | rest grid / tier map / area regions to `StockMesh`, colours | 801 |
| `maps/memo.rs` | `MeshMemo<K,V,CAP>` (Weak mesh identity), `CacheCounters` (private mod) | 223 |
| `maps/geom_cache.rs`, `tier_map_cache.rs`, `reach_map_cache.rs`, `finish_surface_cache.rs` | four bounded memos (capacity 4/2/4/2) | 499 / 320 / 312 / 544 |
| `maps/tool_shape_key.rs` | bit-exact `ToolShapeKey` (`pub(crate)`) | 137 |

### metrology/ (2.7k)
| file | responsibility | lines |
|---|---|---|
| `metrology/mod.rs` | facade; contract doc (4 rules); 6 `pub mod`, no re-exports | 62 |
| `metrology/spacing.rs` | achieved raster spacing, `path_structure`, `quantile` | 362 |
| `metrology/floor.rs` | finish-length floor, `× floor` ratio, `mesh_area_mm2` | 264 |
| `metrology/costing.rs` | `relink_and_cost[_under]`, `CostingContext`, `LinkRegime` | 228 |
| `metrology/census.rs` | coherence + curvature censuses, prize cells, many `pub const` thresholds | 678 |
| `metrology/monge.rs` | Monge-quadric fit, `quantiles`, `median`, `tessellate_heightfield` | 592 |
| `metrology/union_coverage.rs` | `audit_stock_vs_model`, `UnionCoverageFailure` | 477 |

## 2. Public entry points and outside callers (rg, non-test, outside the four folders unless noted)

| entry point | defined | outside callers |
|---|---|---|
| `TriDexelStock` (343 refs core/src, 380 core/tests, 19 viz, 5 cli) | `dexel_stock/mod.rs:88` | `compute/simulate.rs`, `compute/stock_carry.rs`, `adaptive3d/path.rs:778`, `export/viz.rs`, viz `app/simulation.rs` |
| `simulate_toolpath_with_lut_metrics_*`, `_with_lut_cancel*` | `dexel_stock/simulation.rs:361,414,66,85` | `compute/simulate.rs:1609,1688,1759` (rapid_checked variants only there) |
| `simulate_toolpath_range` | `dexel_stock/simulation.rs:884` | viz (5 refs), core 2 |
| `stock_edit::{outline_fill_edits,mesh_cast_edits,apply_column_edits}` | `dexel_stock/stock_edit.rs:160,251,316` | `compute/stock_change_apply.rs:33` only |
| `SimulationTriage::build`, `build_with_rest_context` | `stock/sim_triage.rs:316,339` | `session/compute/diagnostics.rs:902` only |
| `SimulationCutTrace::from_samples*`, `write_simulation_cut_artifact*` | `stock/simulation_cut/analysis.rs:72`, `reporting.rs:207` | `gcode/mod.rs:1638`, `trace/narrate.rs:2263`, cli `job.rs:651`; `simulation_cut` has 135 core refs, 54 viz refs |
| `MeasurabilityReport::from_trace` | `stock/sim_measurability.rs:308` | `session/compute/diagnostics.rs:393,831,1131`, viz `app/mcp/diagnostics.rs:92`, `trace/narrate.rs:2098` |
| `check_collisions*`, `check_rapid_collisions_against_stock`, `RapidClearanceCheck` | `stock/collision.rs:367,403,557,705` | `compute/collision_check.rs:116`, `compute/simulate.rs:21,1755,1855`; viz 20 refs |
| `HolderCollisionCheck` | `stock/collision.rs:283` | viz `state/simulation/playback_state.rs:361`, `session/compute/diagnostics.rs:24` |
| `MaterialSlotTable`, `CutAs`, `judged_as`, `population_view` | `stock/material_slot.rs:79`, `cut_as.rs:27,230,198` | `compute/stock_change_apply.rs:16,300`, viz `app/mcp/commands/stock_changes.rs:83,718` |
| `RadialProfileLUT` (58 core refs) | `stock/radial_profile.rs:23` | dressup, adaptive, compute (not viz) |
| `dexel_stock_to_mesh*`, `z_grid_marching_cubes*` | `stock/dexel_mesh.rs:218`, `dexel_mesh_mc.rs:63` | viz `app/simulation.rs:95,124,431`, `export/fingerprint.rs:704` |
| `StockMesh` (22 core, 30 viz refs) | `stock/stock_mesh.rs:17` | viz render path |
| `cached_tier_map`, `extract_tier_islands`, `compute_tier_map` | `maps/tier_map_cache.rs:215`, `tier_islands.rs:806` | `session/multitool.rs:544,1359,1371` only |
| `cached_reach_map`, `compute_reach_map` | `maps/reach_map_cache.rs:211`, `reach_map.rs:1759` | viz `compute/worker.rs:1841`, `app/mcp/view.rs:47,81`; `session/reach.rs` |
| `geom_cache::cached_{auto_index,silhouette,transform}`, `lazy_auto_index` | `maps/geom_cache.rs:269,338,365,328` | `session/compute/generation.rs:151,755`, `session/reach.rs:136,137,157` |
| `cached_finish_surface` | `maps/finish_surface_cache.rs:337` | `finish/scallop/research.rs:264`, `finish/ramp_finish.rs:554`, `finish/steep_shallow.rs:700` |
| `rest_grid_to_heatmap_mesh`, `tier_map_to_heatmap_mesh`, `area_regions_to_mesh` | `maps/rest_heatmap_mesh.rs:66,267,451` | viz (12 refs) |
| `metrology::*` | all | none in `src/` or other crates; 66 refs in `crates/rs_cam_core/tests/` |

## 3. Dependencies and layering

Imports by target folder (non-test `crate::` mentions):
- stock: stock 66, geo 21, toolpath 14, dexel_stock 12, machine 11, ops 10, compute 10, trace 9, tool_load 9, tool 9, ids 9, budget 8, material 4, finish 3, export 3, session 2, gcode 2.
- dexel_stock: stock 74, geo 15, tool 12, toolpath 10, interrupt 7, trace 4, ops 3, ids 3.
- maps: maps 93, surface 17, geometry 16, tool 14, finish 13, mesh 12, interrupt 9, geo 9, compute 9, stock 6, polygon 4, session 3, feeds 3, adaptive3d 1.
- metrology: geo 9, mesh 6, metrology 5, finish 5, trace 2, toolpath 2, maps 2, machine 2, dexel_stock 2, compute 2.
- External crates: rayon, serde, smallvec, tracing, serde_json. No `thiserror`/`anyhow`.

Possible layering inversions (real `use`/path, not doc links):
| from | to | evidence |
|---|---|---|
| `stock/` (data model) | `compute/` (orchestration) | `stock/sim_triage.rs:284,343,615,735` use `compute::obstacle`, `compute::toolpath_stats::FreshTwinTime`, `compute::catalog::OperationConfig` in public signatures |
| `stock/` | `finish/` | `stock/sim_triage.rs:1008` const aliases `finish::pencil::ENTRY_RAMP_MIN_BITE_MM` |
| `stock/simulation_cut` | `export/` | `stock/simulation_cut/reporting.rs:212,229` call `export::artifact_io` (disk I/O in a data-model file) |
| `stock/simulation_cut` | `machine/` | `stock/simulation_cut.rs:469,568,707,785` embed `machine::kinematics` types in serde structs (the trace schema depends on machine types) |
| `stock/sim_triage` | `machine::kinematic_utilization` | `stock/sim_triage.rs:53` |
| `maps/` | `compute/transform` | `maps/geom_cache.rs:101,119-120` keys a cache on `FaceUp`, `ZRotation`, `SetupTransformInfo` |
| `maps/` | `feeds` | `maps/tool_shape_key.rs:40` (`ToolGeometryHint`) |
| `maps/` | `adaptive3d` | `maps/rest_heatmap_mesh.rs:451` takes `adaptive3d::AreaRegionMap` |
| `maps/` | `stock::stock_mesh`, `stock::collision` | render mesh type; `tier_flute_reach.rs:45` calls collision kernels (CLAUDE.md-sanctioned) |
| `metrology/` | `maps`, `finish`, `dexel_stock` | `union_coverage.rs:61` (`TriDexelStock`); costing uses `finish` relink |
| `stock/` <-> `dexel_stock/` | mutual | `stock/*` imports `dexel_stock` (12); `dexel_stock/*` imports `stock` (74). The two folders are one cycle (`dexel_stock/mod.rs:60-62` uses `stock::dexel`, `stock::material_slot`, `stock::radial_profile`; `stock/dexel_mesh.rs` uses `TriDexelStock`). `stock/mod.rs:5` says "the engine itself is `dexel_stock`". |
Doc-only links that cite higher layers: `metrology/mod.rs:22,32,36`, `maps/reach_map.rs:156,864,1250` (`session`, `export::fingerprint`).

## 4. Traits

| trait | defined | implementors / notes |
|---|---|---|
| `AirCutRatios` (pub) | `stock/simulation_cut.rs:613` | implemented via a local macro at `stock/simulation_cut.rs:657` for the summary types. Provides the two air-cut denominators. |
| `BandJob`, `BandPartial` (`pub(super)`) | `dexel_stock/band_batch.rs:58,66` | `PlaybackJob/PlaybackPartial` `dexel_stock/playback.rs:206,215`; `StampJob/StampPartial` `dexel_stock/whole_path.rs:258,267` |
| `StampTarget` (`pub(super)`) | `dexel_stock/stamping.rs:536` | `DexelGrid` `:546`, `GridBand<'_>` `:575` |
| error types implement `std::error::Error` by hand | `metrology/union_coverage.rs:186`, `maps/tier_map.rs:328`, `maps/tier_islands.rs:248`, `stock/material_slot.rs:72` | `NotJudged` (`stock/cut_as.rs:78`) has `Display` only |
Other impls: `Default` manual for `ReachMapParams` (`maps/reach_map.rs:257`), `TierMapParams` (`tier_map.rs:457`), `TierIslandParams` (`tier_islands.rs:335`), `PlaybackDispatch`, `StampDispatch`, `Bounded<T>` (`stock/sim_triage.rs:87`); manual `Clone for TriDexelStock` (`dexel_stock/mod.rs:136`) and `DexelGrid` (`stock/dexel.rs:473`); `From<Cancelled> for TierMapError` (`maps/tier_map.rs:330`); manual `PartialEq/Eq` for `ReachRequestKey` (`maps/reach_map_cache.rs:119,125`).
Used from elsewhere: `MillingCutter` (`crate::tool`) is the cutter abstraction (`&dyn MillingCutter` in `RadialProfileLUT::from_cutter`, `TierLadder::new`, `ToolShapeKey::new`). `CancelCheck` (`interrupt.rs:5`) is the cancel abstraction; a closure implements it (`interrupt.rs:9`).
Deliberately not used: no `OpParams`/`ParamEnum` derive (these are not operations); no `thiserror`; no trait for "a map" (`TierMap` and `ReachMap` share only `GridSpec`); no `Default` derive on the three params structs (hand `impl Default`).

## 5. Recurring-pattern table

| concern | pattern used here | example path:line | enforcement | deviations inside the area |
|---|---|---|---|---|
| errors and refusals | Cancellation is `Result<_, Cancelled>` (unit struct from `interrupt`). Domain refusals are small hand-written enums/structs with manual `Display`+`Error`. No `anyhow`, no string errors, no `thiserror`. Capacity refusal is a struct carrying the rejected value. | `stock/material_slot.rs:56,61,72,102`; `maps/tier_map.rs:298-330`; `maps/tier_islands.rs:198-248` | LINT (workspace denies `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`: `Cargo.toml:37-41`) + TYPES (`Result`, `#[must_use]`) | `metrology/union_coverage.rs:192` returns `Result<(), Box<UnionCoverageFailure>>` (an assertion helper, not a refusal). `stock/cut_as.rs:78` `NotJudged` is an error-like type with no `Error` impl. `stock/simulation_cut.rs:835,841` serde `Result<S::Ok,S::Error>` helpers. Zero production `unwrap/expect/panic` found (all hits are in `#[cfg(test)]`). Absent data is `Option`, absence encoded by `None` vs `Some(0.0)` (rule in `stock/CLAUDE.md`, `metrology/mod.rs:15`) |
| state mutation | The area owns data. It does not use `ProjectSession::apply`/`Command`. Stock is mutated by `&mut TriDexelStock` methods and by direct pub-field writes. `stock_edit::apply_column_edits` is the S3 mutation entry. | `dexel_stock/mod.rs:88-120` (all fields `pub`); `dexel_stock/stock_edit.rs:316` | CONVENTION | `TriDexelStock`, `DexelGrid` (`stock/dexel.rs:359-413`) fields are all `pub`; `DexelGrid::ray_mut` (`stock/dexel.rs:620`) is `pub` and is called by `adaptive3d/path.rs:778,819` outside the area, bypassing the `conservative_top` upkeep (doc says a stale high value fails safe, `stock/dexel.rs:~395`). `last_stamp_dispatch` / `last_playback_dispatch` are pub diagnostics fields written by the engine (`dexel_stock/mod.rs:101,126`) |
| parameters and config | Hand-written `*Params` structs with `impl Default` and doc-stated bounds; clamps inside constructors/accessors; consts for ceilings. No `OpParams`, no derive-written schema, no central validate. | `maps/reach_map.rs:245-320` (`ReachMapParams::for_cutter`); `maps/tier_islands.rs:278-400` (`clamp_coarseness`, `effective_*`); `maps/tier_map.rs:436-470` | CONVENTION (+ TYPES for enum choices such as `ResidualTreatment`, `ReachToleranceSource`) | Each params struct has its own clamp style: `maps/tier_islands.rs:400`, `stock/dexel.rs:515` (`clamp_cell_size`), `maps/reach_map.rs:214,227` (`MIN/MAX_REACH_CELL_MM`). Dispatch knobs are pub fields on the stock (`dexel_stock/mod.rs:96,108`) |
| units and frames | mm everywhere (suffix `_mm`/`_mm2`/`_s`/`_pct` in names; `_deg` for angles), bare `f64`. Dexel Z values are `f32` (`DexelSegment` enter/exit). Frames are in prose: "world", "setup-local". Lateral setups do not enter the global stock (G-LATERALSCRUB). Dexel Z is negative for 2D ops (`origin_z` negative). | `stock/dexel.rs:20-50`; `dexel_stock/mod.rs:15-30,110-125`; `dexel_stock/CLAUDE.md:8-11` | CONVENTION (names) + DOC. `StockCutDirection`/`DexelAxis` are enums (TYPES) | No unit or frame newtype. `f32`/`f64` boundary is converted ad hoc (`maps/reach_map.rs` 33 `f32` mentions, `dexel_stock/stamping.rs` 38). `FaceUp`/`ZRotation` appear only in `maps/geom_cache.rs:119`. Drill API (`dexel_stock/mod.rs:685-735`) documents "relative to whatever frame the caller holds" |
| identifiers | `ToolpathId` (newtype, `crate::ids`) in stock keys; `MaterialSlot(u8)` newtype; `DiagnosticId`, `SpanId`. Mesh identity is `Weak<TriangleMesh>`/`Arc::ptr_eq` or content hash. | `stock/cut_as.rs:18`; `stock/material_slot.rs:32`; `maps/reach_map_cache.rs:103-125` | TYPES for newtypes; DOC for cache identity | `ReachMap.tool_id: usize`, `model_id: usize`, `with_ids(usize, usize)` (`maps/reach_map.rs:327-328,480-482,728`; also `maps/reach_map_cache.rs:69-70`) are bare usize although `ids.rs` has newtypes. `MaterialSlot(pub u8)` has a public field so `MaterialSlot(9)` compiles past the capacity of 8 (`stock/material_slot.rs:32`). `POPULATION_EXCLUDED = ToolpathId(usize::MAX)` and `NO_TIER = u8::MAX` (`maps/tier_map.rs:149`) are in-band sentinels. Tier index is bare `u8`/`usize` |
| background work, progress, cancel | Every long kernel takes `cancel: &dyn CancelCheck` (usually `+ Sync`), polls per row/band/batch, returns `Cancelled`; `NeverCancel` for non-cancel wrappers. Parallelism is rayon behind feature `parallel`. | `maps/grid.rs:94-137`; `dexel_stock/band_batch.rs:53,275`; `stock/collision.rs:403` | TYPES (the `Result<_,Cancelled>` return forces the caller to handle it) | Cancel poll granularity differs: per grid row (`maps/grid.rs`), per band batch (`dexel_stock/band_batch.rs`), per move (`dexel_stock/simulation.rs`). No progress reporting in the area. Non-cancel wrappers are separate fns (`simulate_toolpath` vs `_with_cancel`, 9 variants: `dexel_stock/simulation.rs:39-907`) |
| caching, freshness, invalidation | `MeshMemo<K,V,CAP>`: key = mesh identity + request key, `Weak` mesh, dead sweep, oldest-first eviction. Four statics with tiny capacities. Identity by pointer (Weak, three caches) or by content hash (finish surface). Tool identity is bit-exact `ToolShapeKey`. | `maps/memo.rs:110-206`; `maps/tool_shape_key.rs:74-95`; `maps/finish_surface_cache.rs:136-170` | TYPES (shared `MeshMemo`) + SENTRY (`tier_map_cache_t3`, `the_test_doors_are_gated_fld0405`) | `finish_surface_cache` uses `DefaultHasher` content digest (`:136,168`), the others use `Weak`. `ToolShapeKey::new` lists the cutter queries by hand (`maps/tool_shape_key.rs:85-93`): a new shape-relevant `MillingCutter` method is not forced into the key by the compiler. Each cache repeats `stats/reset_stats/cache_len/clear` + a `*CacheStats` struct (`geom_cache.rs:167-236`, `tier_map_cache.rs:146-196`, `reach_map_cache.rs:143-193`, `finish_surface_cache.rs:265-314`). `stock` caches nothing; staleness of a sim trace is judged elsewhere (`gcode::sim_trace_is_fresh`, cited at `stock/simulation_cut.rs:133`) |
| diagnostics, warnings, messages | Structured-first: enums with a `describe()`/`label()`/`as_str()` method, then free text. Findings use the shared `Diagnostic`/`DiagnosticId`/`Severity` contract. Reports carry notes as `String` helpers. | `stock/sim_measurability.rs:77-148,156-232`; `stock/sim_triage.rs:129-233`; `maps/reach_map.rs:550-610` (`area_basis_note`, `grid_note`) | TYPES for ids (`diagnostics::ids::*` consts) ; CONVENTION for wording | `ChannelCounts::from_trace` etc. embed literal `0.02`/`0.10` (see section 9). `SimMetric::as_str` and `label` are two parallel `match`es (`stock/sim_measurability.rs:126,139`). Rationale strings built with `format!` inside `describe()` (`:196-230`) |
| serialization and wire types | `serde` derive on report/trace types; `rename_all = "snake_case"`, `tag = "..."` for payload enums; `#[serde(default)]` on added fields; schema const `SIMULATION_CUT_TRACE_SCHEMA_VERSION = 6`. `ReachMap.grid` is `#[serde(flatten)]`. Artifact JSON written by `write_json_artifact`. | `stock/simulation_cut.rs:28,31,153-250`; `stock/sim_measurability.rs:155`; `maps/reach_map.rs:364-370` | CONVENTION; SENTRY for the trace (`gcode`/`session` load tests, not in area) | Dexel types, `TierMap`, `TierIslands` are not serde (internal). `StockMesh` itself is not serde but `DisplayMeshDegrade` is (`stock/stock_mesh.rs:133,146`). MCP wire types live in `rs_cam_mcp`, not here; viz/MCP re-wrap these |
| numeric tolerances | Named `pub const`s with doc, in the file that owns them; a few shared across files. Many inline literals remain (about 188 `1e-N`/`0.0001` literal hits in non-test code across the four folders). | `stock/collision.rs:332` (`BODY_STRIKE_THRESHOLD_MM`); `dexel_stock/stamping.rs:331`; `metrology/census.rs:29-111`; `maps/reach_map.rs:148,214,227,235,240` | CONVENTION | Air-cut/low-engagement thresholds `0.02`/`0.10` repeated as literals in 4 files despite `AIR_CUT_RADIAL_WOC_FRACTION` (`stock/simulation_cut.rs:253`): `stock/sim_triage.rs:199-201`, `stock/simulation_cut/analysis.rs:161-163`, `reporting.rs:110-112`, `accumulate.rs:135-138`. `monge.rs` has `EPS_DENOM` and `EPS_VEC` both 1e-12 (`:56,58`). `STANDING_MATERIAL_SAMPLE_FRACTION = 0.02` (`stock/sim_triage.rs:669`) is a different 0.02 |
| geometry primitives | `crate::geo::{P2,P3,BoundingBox3,Triangle}`, `TriangleMesh`, `SpatialIndex`, `Polygon2`. Local grid math is `usize` row/col + `f64` origin/cell. | `dexel_stock/mod.rs:60`; `maps/grid.rs:43-90`; `stock/dexel.rs:587-612` | CONVENTION (CLAUDE.md in metrology: primitives belong to `geo.rs`) | Local copies of primitives: `maps/reach_map.rs:1733` `triangle_area`, `:1740` `triangle_centroid`, `:1375` `finite_quantile`; `metrology/monge.rs:527` `median`, `:488` `quantiles`; `metrology/spacing.rs:211` `quantile`; `metrology/floor.rs:200` `mesh_area_mm2`. Probes use `(f64,f64,f64)` tuples (`dexel_stock/band_batch.rs:60-61`, `DexelAxis::decompose`) instead of `P3` |
| test style and sentries | Inline `#[cfg(test)] mod tests` at file bottom with `#[allow(clippy::unwrap_used, ...)]`; integration sentries in `crates/rs_cam_core/tests/` named `<what>_<ticket>`; test doors are `#[cfg(feature = "test-support")]` and `#[cfg(test)] pub(crate)`. | `dexel_stock/mod.rs:837-839`; `maps/tier_map_cache.rs:143-196`; `stock/sim_triage.rs:237-243` | SENTRY (all 31 named sentries exist) ; LINT for door gating (`the_test_doors_are_gated_fld0405`) | `stock/simulation_cut/tests.rs` (1310) and `stock/dexel_mesh_mc/stride_tests.rs` (659) are split files; most others inline. `maps/reach_map.rs` has no inline tests (tests in `tests/reach_map_*`). `tier_map::drop_call_count` (`:164`) is ungated while `reset_drop_call_count` (`:176`) is gated |
| naming and layout | `snake_case` files named by concept + ticket tags in docs (STK-nn, G-xxx, S1..S6). Folder `mod.rs` facades: `stock` and `metrology` declare all, `dexel_stock` keeps modules private and re-exports a few items, `maps` mixes `pub mod` and private `mod memo` / `pub(crate) mod tool_shape_key`. | `stock/mod.rs`; `dexel_stock/mod.rs:42-69`; `maps/mod.rs` | CONVENTION | `metrology/CLAUDE.md` says `mod.rs` "re-exports each ruler"; it only declares. `stock` holds engine-adjacent code (collision, meshes) and `maps` holds rendering (colour ramps, meshes) |
| registration of a new feature | Closed enums with exhaustive `match`es plus parallel `ALL` arrays and string-key methods. | `stock/sim_measurability.rs:77-148` (`SimMetric`) ; `stock/cut_as.rs:27-55` (`CutAs`) ; `maps/reach_map.rs:159-205` (`ReachToleranceSource`) | TYPES for `match`; CONVENTION for `ALL` arrays and string keys | `SimMetric::ALL` (`:108`) and `ENGAGEMENT_DERIVED` (`:120`) are hand-kept arrays (no exhaustiveness check). `CutAs::token`/`label`, `SimMetric::as_str`/`label`, `ReachToleranceSource::key` return strings that other crates parse (string keys). `MAX_TIERS = 255` with `NO_TIER = 255` conflict by design (`maps/tier_map.rs:149-152`) |

## 6. Extension points

| to add | files touched | compiler forces |
|---|---|---|
| a new `SimMetric` | `stock/sim_measurability.rs` (variant, `ALL`, maybe `ENGAGEMENT_DERIVED`, `as_str`, `label`, `from_trace` branch ~`:308-350`); consumers: `session/compute/diagnostics.rs`, `trace/narrate.rs:2098`, viz `ui/sim_diagnostics.rs:2018-2020` | `as_str`/`label` matches and `Measurability` consumers that match exhaustively. NOT forced: `ALL` array, `from_trace` content, narrate/viz lists |
| a new `MeasurabilityReason` | `stock/sim_measurability.rs:156` + `describe()` `:196` + `from_trace` | `describe()` match; serde tag shape not checked |
| a new trace sample field | `stock/simulation_cut.rs:153` (`SimulationCutSample`, add `#[serde(default)]`), `test_fixture` `:268`, accumulators `simulation_cut/accumulate.rs`, producer `dexel_stock/simulation.rs`, `SIMULATION_CUT_TRACE_SCHEMA_VERSION` bump `:28` | struct-literal construction sites (`sample_coalesce.rs:412`, tests); not the schema bump or the loaders |
| a new triage finding | `stock/sim_triage.rs` (finding fn + wire into `build_with_rest_context` `:339`), `diagnostics::ids` const, severity/class, dedup key; add measurability gate | none (a new fn is not forced into the builder); `category_rank` match `:1805` if a new `Category` |
| a new `CutAs` | `stock/cut_as.rs:27` (+`label`, `token`), `compute/stock_change_apply.rs`, viz `app/mcp/commands/stock_changes.rs:83` | the two matches; not the string parsers |
| a new material slot rule | `stock/material_slot.rs` (`MATERIAL_SLOT_CAPACITY` 8 sized into `SlotVolumes([f64;8])`, `SlotTally`) | array length ties to the const; `MaterialSlot(pub u8)` unguarded |
| a new tool kind in the cache key | `maps/tool_shape_key.rs` (`HintKey`, `ToolShapeKey::new`), `tool/` impl | `HintKey::new` match on `ToolGeometryHint` is exhaustive (TYPES). A new non-hint cutter query is NOT forced into `ToolShapeKey` |
| a new map kind | new file in `maps/`, `maps/mod.rs` line, a `*_cache.rs` using `MeshMemo` + stats/clear doors (copy of 4 existing), `GridSpec` field, `walk_rows`, heat-map in `rest_heatmap_mesh.rs`, callers in `session/` or viz worker | nothing |
| a new tier-map param | `maps/tier_map.rs:436` (`TierMapParams` + `Default`), cache key struct in `tier_map_cache.rs`, `session/multitool.rs:1359` | key completeness is not forced (stale-answer risk if the key omits it; sentry `tier_map_cache_t3`) |
| a new ruler | file in `metrology/`, `mod.rs` line, a test in `crates/rs_cam_core/tests/`, `metrology/CLAUDE.md` sentry list | nothing |
| a new `simulate_*` variant or stamp route | `dexel_stock/simulation.rs`, `whole_path.rs` or `playback.rs` (job/partial impl of `BandJob`/`BandPartial`), `band_batch.rs` | the two traits force the replay shape; the "metric route == playback route volume" invariant is a SENTRY only (`playback_band_dispatch_s6`) |
| a new `DexelAxis` / `StockCutDirection` | `stock/dexel.rs:327` (`DexelAxis::decompose`), `dexel_stock/cut_direction.rs:11`, `stock/dexel_mesh.rs:173-208,361` | exhaustive matches; side grids are test-reachable only |

## 7. CLAUDE.md drift

| statement | actual |
|---|---|
| `metrology/CLAUDE.md`: "`mod.rs` ... re-exports each ruler" | `metrology/mod.rs:57-62` only declares `pub mod`, no `pub use` |
| `stock/CLAUDE.md`: colours live in `export/` | `stock/dexel_mesh.rs:19-24` still defines `UNCUT_*`/`CUT_*` RGB consts (`pub(crate)`); `export/material_colour.rs:241` imports them back |
| `stock/CLAUDE.md` file list | omits `stock/dexel_mesh_mc/stride_tests.rs` (659 lines) and the `simulation_cut/` file split roles beyond "cut trace, assembly, reports" |
| `dexel_stock/CLAUDE.md`: "`stock_edit.rs` ... S3 scans" ; one cell walk `walk_cell_box` used by stock_edit | true (`stock_edit.rs:17`). But `stock_edit` is the only `pub mod` in the folder (`dexel_stock/mod.rs:41`) and the only item the CLAUDE.md folder-private idea does not cover |
| `dexel_stock/CLAUDE.md` invariant "0.02 mm Z subdivision" | matches `sample_coalesce.rs:62`; no sentry links the const to the claim |
| `dexel_stock/CLAUDE.md` sentry `the_six_cell_loops` "runs ONLY under `--lib`" | test at `dexel_stock/stamping.rs:2450` (inline); true |
| `maps/CLAUDE.md`: "`reach_map_for_mesh`, `stats`, `reset_stats`, `cache_len`, `clear` are `test-support` doors" | true for `reach_map_for_mesh` (`reach_map.rs:1852`) and the stats set; but `tier_map::reset_drop_call_count` (gated, `:176`) is not listed and `drop_call_count` (`:164`) is ungated public |
| `maps/CLAUDE.md`: "the flute-reach pass calls `stock::collision::body_segment_penetration_mm`" | `maps/tier_flute_reach.rs:45` imports from `stock::collision`; consistent |
| root rule: "Every production `allow(...)` carries a `// SAFETY:` line" (root `CLAUDE.md`) | about 86 production-region `#[allow(...)]` in the four folders; only about 9 have a `SAFETY:` line (`metrology/monge.rs:165-524`, `maps/reach_map.rs:1866`, `stock/radial_profile.rs:115`, `dexel_stock/swept.rs:706`). The rest use a trailing "bounded indexing in algorithmic code" or no comment (`dexel_stock/simulation.rs:126,271,322`, 17 in that file; `stamping.rs` 15; `swept.rs` 12; `stock/dexel.rs` 16) |
| `metrology/mod.rs:32` says air-cut tables name their denominator | the 5 literal `0.02`/`0.10` sites do not cite either |
| all 31 sentries named in the four CLAUDE.md files exist under `crates/rs_cam_core/tests/` | verified present; none missing |

## 8. Navigability

| item | note |
|---|---|
| `dexel_stock/stamping.rs` 3081 lines (prod ~2095) | per-cell kernels, `StampTarget`, cell-walk driver, bbox clamp, determinism helpers, and the engagement/volume accounting in one file |
| `stock/sim_triage.rs` 2581 (prod ~1815) | one file holds the builder, 5+ finding rules (standing-material, entry-load G-ENTRYLOAD, link-cut G-LINKPLOUGH, plunge-class, collision, rest-slower), dedup/cap/sort. Candidate split by rule. Contains observation structs that are `pub` |
| `dexel_stock/simulation.rs` 2068 (prod ~1815) | nine `simulate_*` entry variants + chip-thickness stats; 17 `allow`s |
| `maps/reach_map.rs` 1900, no inline tests | compute kernel, params/request, tolerance policy text, colour ramp, overlay mesh and note strings together |
| `maps/tier_islands.rs` 1581, `dexel_stock/mod.rs` 1612 (stock type + probes + drill op + a 770-line test module), `stock/dexel.rs` 1483 (types + ray ops + grid + window + mask) | over or near 1500; `dexel_stock/mod.rs` prod is ~840 |
| Name: `dexel_stock/` vs `stock/dexel.rs` | the engine and the data types are in different folders; `stock/` also holds meshes, collision, triage: the folder is "everything about stock" |
| Name: `lateral` in `dexel_stock/simulation.rs:487` | means `StampDispatch::Swept`, not a lateral (side) setup (`FaceUp`) which the module doc calls "lateral"; two senses |
| Name: `maps/` | holds a mesh renderer (`rest_heatmap_mesh.rs`, colour ramps in `reach_map.rs:821-912`) and cache infra; the dependency on `stock::stock_mesh` makes it a render layer |
| Wrong folder | `stock/simulation_cut/reporting.rs:207-249` (artifact file write/prune) belongs with `export/artifact_io`; `stock/collision.rs` (cutter vs stock) is engine logic that `maps/tier_flute_reach.rs` depends on; `maps/geom_cache.rs` (index/silhouette/transform memo) is not a map; `metrology/costing.rs` depends on `finish` relink and `machine` |
| Dead-end folder | `metrology/` has zero production callers (section 9) |

## 9. Candidate debt leads (UNVERIFIED)

1. `stock/sim_triage.rs:199-201`, `stock/simulation_cut/analysis.rs:161-163`, `stock/simulation_cut/reporting.rs:110-112`, `stock/simulation_cut/accumulate.rs:135-138`: the same `< 0.02` / `< 0.10` radial-WOC classification written four times as literals. Only the 0.02 has a const (`stock/simulation_cut.rs:253`). No const for the 0.10 low-engagement bound.
2. `metrology/*` is compiled into the product lib but no non-test, non-metrology code in any crate calls it (`rg 'metrology::'` outside `core/tests` returns none). Either a test-only instrument module that should be feature-gated, or dead code.
3. `maps/reach_map.rs:327-328,480-482,728`, `maps/reach_map_cache.rs:69-70`: `tool_id`/`model_id` are bare `usize`; `ids.rs` newtypes exist.
4. `stock/material_slot.rs:32`: `MaterialSlot(pub u8)` lets any caller build a slot beyond `MATERIAL_SLOT_CAPACITY` (8). `SlotVolumes([f64; 8])::add(slot)` then indexes by `slot.index()` (check for out-of-range handling).
5. `stock/dexel.rs:620` `DexelGrid::ray_mut` + all-`pub` fields on `DexelGrid`/`TriDexelStock`: callers outside the area (`adaptive3d/path.rs:778,819`) mutate rays without updating `conservative_top`. Safe only by the "stale high fails safe" argument.
6. `maps/tool_shape_key.rs:85-93`: the cache key lists cutter queries by hand; no compile-time link to the `MillingCutter` trait surface.
7. Four near-identical cache API blocks (`stats/reset_stats/cache_len/clear` + `*CacheStats`): `maps/geom_cache.rs:167-236`, `tier_map_cache.rs:146-196`, `reach_map_cache.rs:143-193`, `finish_surface_cache.rs:265-314`. Could be one generic wrapper over `MeshMemo`.
8. Statistics helpers duplicated: `metrology/monge.rs:527 median`, `:488 quantiles`; `metrology/spacing.rs:211 quantile`; `maps/reach_map.rs:1375 finite_quantile`; also outside: `surface/rest_field.rs:1370 median_sorted`, `tool_load/distribution.rs:301 percentile`, `finish/conformal_spiral.rs:1887,1948`. Point-segment distance copies outside the area contradict `metrology/CLAUDE.md` (`geo.rs:381,393` is the owner): `adaptive/search.rs:844`, `compute/alignment_pins/walls.rs:181`, `compute/execute/standing_material.rs:764`, `dressup/link.rs:49`, `polygon.rs:1946`.
9. `maps/reach_map.rs:1733,1740`: local `triangle_area`/`triangle_centroid`; `metrology/floor.rs:200` `mesh_area_mm2`: three triangle-area computations.
10. `dexel_stock/simulation.rs:39-907`: nine `simulate_*` variants differing by cancel / LUT / metrics / rapid_checked flags; several take 8+ args (17 `too_many_arguments` allows). Positional flag/option API; outside use is concentrated in `compute/simulate.rs`. Variants `simulate_toolpath_with_cancel` (2 refs) and `simulate_toolpath_range_with_lut` (3 refs) look near-unused.
11. Layering: `stock/sim_triage.rs` depends on `compute` (`:284,343,615,735`) and `finish` (`:1008`); `stock/simulation_cut.rs:469,568,707,785` embed `machine::kinematics` types in the serde trace; `stock/simulation_cut/reporting.rs:212,229` does file I/O via `export`.
12. `stock/sim_measurability.rs:108-123`: `ALL` and `ENGAGEMENT_DERIVED` are hand-maintained arrays beside the enum; no exhaustiveness check on `ALL`.
13. `maps/tier_map.rs:149-152`: `NO_TIER = u8::MAX` and `MAX_TIERS = 255` in-band sentinel; `stock/cut_as.rs:245` `POPULATION_EXCLUDED = ToolpathId(usize::MAX)` sentinel instead of `Option`.
14. `dexel_stock/CLAUDE.md` and `dexel_stock/mod.rs:9-36`: side grids (`x_grid`, `y_grid`) and two branches in `stock/dexel_mesh.rs` are declared dead in production but kept; they carry test-only surface (STK-13 ruling pending per memory).
15. `stock/CLAUDE.md` "None means not measured; Some(0.0) measured zero" is DOC-only; `Option<f64>` fields are typed but the rule across summary structs (`stock/simulation_cut.rs:407-540`) is not checked by any lint.
16. `stock/dexel_mesh.rs:19-24` colour consts with `pub(crate)` imported by `export/material_colour.rs`; colour logic is split between `stock`, `maps/rest_heatmap_mesh.rs` (`:60,157,218,227,436`), `maps/reach_map.rs:821-870`, and `export/`.
17. `maps/finish_surface_cache.rs:136,168` uses `DefaultHasher` (not stable across Rust versions; fine in-process) for content identity while the sibling caches use `Weak` pointer identity; two identity schemes in one folder.
18. `stock/sim_measurability.rs:282,287` `NOT_MEASURABLE_BLIND_FRACTION` 0.5 / `DEGRADED_BLIND_FRACTION` 0.1 are `pub(crate)` consts whose 0.1 echoes the unnamed 0.10 low-engagement literal (lead 1); check they are intentionally separate.
19. 86 production `#[allow]` lines in the area mostly `indexing_slicing` / `too_many_arguments` with no `SAFETY:` line (drift row above). Workspace denies `indexing_slicing`, so the allow count is a proxy for unchecked indexing hot spots (`dexel_stock/stamping.rs` 15, `swept.rs` 12, `stock/dexel.rs` 16).
