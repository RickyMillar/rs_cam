# Map: core-finish (`crates/rs_cam_core/src/finish/`)

Baseline HEAD 99b6fb41. Read-only review. All paths are relative to `crates/rs_cam_core/src/` unless they start with `crates/`, `planning/` or `tests/`.
"In flux" = listed in `instruments/in_flux_paths.txt`. No file under `finish/` is in flux. Two sentry tests that read it are: `crates/rs_cam_core/tests/unified_finish_dropped_band_finding_d1.rs` and `.../unified_finish_tapered_end_to_end_m21.rs`. Callers outside the area in `session/`, `compute/` and `crates/rs_cam_cli/` are mostly in flux; re-check them before you edit.
Line counts below include the inline `#[cfg(test)]` tail.

## 1. Purpose and file map

The folder holds every 3D finishing strategy and the unified planner. A strategy takes a mesh, a spatial index, a cutter and a params struct. It returns a `Toolpath` plus a report and runtime annotations. The compute runner (`compute/execute/finish_3d.rs`, `finish_raster.rs`, `composed.rs`) calls them. The folder also owns surface analysers that read finishing code (`classify_probe`, `crest_lines`, `pencil_dihedral`, `direction_field`; `finish/mod.rs:4-8`). About 22 % of the lines (about 7.5k) are research arms behind `--features research`.

Total 34 470 lines in 36 files (`wc -l`), 5 of them test files.

| File | Responsibility | Lines |
|---|---|---|
| `mod.rs` | module list; 3 research mods gated by `cfg(feature="research")` (`mod.rs:15,19,32`) | 38 |
| `composed.rs` | `ComposedSurfaceOp` trait, `compose`, `composed_spans`, `ComposedRest` (W6.4) | 170 |
| `rest_detector.rs` | `detect_rest`, `RestReferenceChoice`, `RestTier`, probe-ball constants | 140 |
| `finish_setup.rs` | `FinishResolutionMode/Policy`, `FinishSurface`, 6 surface builders, slope-filter constants, `SURFACE_BUILDS` counter | 664 |
| `classify_probe.rs` | `ClassificationSampler` (5 arms), `sample_classification_grid`; production = `TileRaster` | 1072 |
| `finish_planner.rs` | `decompose`: slope bands, hysteresis, close, min-area absorb, crease corridors; `FinishBand`; mask helpers | 2002 (prod ~1217) |
| `unified_finish.rs` | Unified Finish: params, claims types, report types, spans, resolution policy, orchestrator, `UnifiedFinishOp` | 2499 |
| `unified_finish/band_kernels.rs` | per-band cut kernels `cut_very_steep/mid_steep/shallow` | 445 |
| `unified_finish/region_routing.rs` | `route_greedy`, link candidates, `band_rank`, preamble strip | 633 |
| `unified_finish/tests.rs` | unit tests | 1707 |
| `steep_shallow.rs` | Steep and Shallow op: waterline half + raster half, `SteepShallowOp` (composed) | 1766 (prod ~1009) |
| `scallop.rs` | Scallop: params, ring-policy enums, report, entry points | 1162 |
| `scallop/ring_generation.rs` | ring decimation, lift, chord refine, cascade | 712 |
| `scallop/research.rs` | ring budget seam, iso-field entry point, region-aware builder. NOT feature-gated | 879 |
| `scallop_isofield.rs` | `IsoScallopField` level-set rings | 599 |
| `scallop_math.rs` | flat/curved scallop height and stepover formulas | 342 |
| `pencil.rs` + `pencil/{detectors,chain_paths,emission}.rs` | Pencil params, detectors (Dihedral/Curvature/RestDepth), chaining, emission, entry ramp, link lift | 695+419+431+970 |
| `pencil_dihedral.rs` | dihedral crease detection | 634 |
| `crest_lines.rs` | curvature crest lines | 1083 |
| `crease_paths.rs` | rest centreline to cut paths (`centerline_cut_paths`) | 398 |
| `surface_link.rs` | `build_surface_link`, `relink_fragments`, `FinishingLinkStage`, `LinkCeiling`, `shape_under_ceiling`, `push_link_landing` | 2108 (prod ~1373) |
| `horizontal_finish.rs`, `radial_finish.rs`, `spiral_finish.rs`, `ramp_finish.rs` | single-strategy finishes | 647, 586, 773, 1320 |
| `conformal_spiral.rs` (+ `/spiral_build.rs`, `/tests.rs`) | research: conformal spiral | 2981 (+507+934) |
| `direction_field.rs` | research: direction field | 2062 |
| `spiral_finish_compact.rs` | research: compact spiral, `CompactSpiralRefusal` | 947 |

## 2. Public entry points and external callers

Counts are files outside `finish/` with `finish::<mod>` (rg). "tests" = `crates/rs_cam_core/tests/` + benches.

| Entry point | Defined | Product callers outside area (path:line) | Files in tests/benches |
|---|---|---|---|
| `unified_finish::UnifiedFinishOp` + `compose` | `unified_finish.rs:1767` | `compute/execute/finish_3d.rs:164`, `compute/execute/composed.rs:144` | 43 |
| `unified_finish::unified_finish_toolpath_with_cancel` | `unified_finish.rs:1427` | `compute/` (10 files ref `unified_finish`) | in the 43 |
| `unified_finish::{CreaseReference, MonotoneCellTotals, RelinkTotals, ClaimsReference}` | `unified_finish.rs:358,1187,1220,408` | `diagnostics/adapters/from_generation.rs:458`, `session/diagnostics_types.rs:94`, `trace/narrate.rs:188`, `crates/rs_cam_cli/src/project.rs:104`, `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:755` | few |
| `steep_shallow::SteepShallowOp`, `steep_shallow_generation_resolution` | `steep_shallow.rs:624,525` | `compute/execute/finish_3d.rs:317,323` | 8 |
| `pencil::*` (`pencil_toolpath_structured_annotated_with_cancel`, `PencilDetector`, `PencilStock`, `plan_entry_ramp`, `tip_contact_radius`) | `pencil.rs:618,71`; `pencil/emission.rs:84` | `compute/execute/finish_3d.rs:46`, `dressup/mod.rs:941`, `dressup/entry_descent.rs:520-541`, `session/compute.rs:1021`, `session/project_file.rs:1418`, `session/mutation/config.rs:258`, `stock/sim_triage.rs:1008`, `crates/rs_cam_viz/src/app/mcp/commands.rs:2959`, `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:933` | 21 |
| `scallop::{scallop_toolpath_*, ScallopParams, ScallopRuntimeAnnotation}` | `scallop.rs:1060,1099,1138`; `scallop/research.rs:84` | `compute/execute/finish_raster.rs:119,130`, `trace/transform_provenance.rs:503-563` | 18 |
| `scallop_math::*` | `scallop_math.rs:15-147` | `metrology/floor.rs:37,249`, `session/reach.rs:95` | 10 |
| `surface_link::{relink_fragments, LinkCeiling, FinishingLinkStage, LinkGeometry}` | `surface_link.rs:835,121,456,428` | `compute/execute/composed.rs:23`, `compute/execute/finish_raster.rs:271,471`, `metrology/costing.rs:35,183,198` (10 compute files) | 18 |
| `finish_setup::*` surface builders | `finish_setup.rs:380-571` | `maps/finish_surface_cache.rs:140,401`, `measurement.rs`, `ops/`, `compute/` | 31 |
| `finish_planner::{decompose, FinishPlannerParams, and_masks_in_place, label_components, morphological_close}` | `finish_planner.rs:390,247,713,728,798` | `maps/tier_islands.rs:100,1328`, `compute/` (2) | 37 |
| `rest_detector::detect_rest` | `rest_detector.rs:118` | `compute/execute/dressup_apply.rs:117` (+ internal `unified_finish.rs:1554`, `pencil/detectors.rs:234`) | 0 |
| `ramp_finish`, `spiral_finish`, `radial_finish`, `horizontal_finish` | see section 1 | `compute/` only (7, 3, 2, 2 refs); `trace/` for `RampReachClamp` (`trace/narrate.rs:131`) | 7, 1, 3, 3 |
| `classify_probe::ClassificationSampler` | `classify_probe.rs:103` | `compute/` (1) | 17 |
| `crest_lines`, `pencil_dihedral`, `crease_paths` | | no code caller outside; `surface/rest_field.rs` and `geo.rs:315` only name them in doc links | 2, 0, 1 |

`surface/rest_field.rs:2868-2906` uses `finish::rest_detector` constants and `finish::pencil::NOMINAL_REFERENCE_BALL_LENGTH_MM` inside its `cfg(test)` tail (from line 2149). That is a test-only edge.

The strategy `pub fn` entry points that no product code calls (only tests or benches): `steep_shallow_toolpath_split_with_cancel` (`steep_shallow.rs:566`; 4 test files + `benches/perf_suite.rs`), `steep_shallow_toolpath_split_with_resolution` (`:595`), `ramp_finish_toolpath_structured_annotated_with_resolution` (`ramp_finish.rs:531`), `decompose_surface` (`finish_planner.rs:610`), `build_finish_surface_with_cell_size_and_cancel` (`finish_setup.rs:422`; used by `scallop/tests.rs` only). Verify before you delete; rg only.

## 3. Dependencies and layering

Imports (non-test) from other folders of `rs_cam_core`:

| Imports | From files (examples) |
|---|---|
| `geo`, `geometry` (`region_set`, `grid_field`, `grid2`, `monotone_cells`, `region_mask`), `polygon`, `mesh`, `tool`, `toolpath`, `interrupt` | almost every file |
| `surface` (`slope`, `rest_field`, `dropcutter`, `chord_refine`, `pushcutter`) | `finish_planner.rs:84-87`, `rest_detector.rs:35-39`, `unified_finish.rs`, `scallop/ring_generation.rs:12` |
| `trace` (`debug_trace`, `produced`, `toolpath_spans`) | `composed.rs`, `pencil.rs:48`, `ramp_finish.rs`, `steep_shallow.rs`, `surface_link.rs` |
| `compute` (`spans`, `toolpath_stats`) | `composed.rs:34`, `steep_shallow.rs:19`, `pencil.rs:42`, `pencil/chain_paths.rs:10`, `pencil/detectors.rs:13`, `unified_finish/band_kernels.rs:10`, `unified_finish.rs:1320` |
| `ops` (`waterline`, `depth`) | `steep_shallow.rs:28,181`, `ramp_finish.rs:23,598`, `unified_finish/band_kernels.rs:20` |
| `machine::kinematics`, `measurement`, `dexel_stock` | `composed.rs:35`, `finish_setup.rs:30`, `rest_detector.rs:34` |

Layering inversions and cycles (all are intra-crate, so the compiler accepts them):

| Edge | Evidence | Note |
|---|---|---|
| `finish -> compute` for data types and span builders, and `compute -> finish` for the runners | finish: `pencil.rs:42` (`TipFloatFinding`), `unified_finish/band_kernels.rs:10` (`BandHeightClip`), `unified_finish.rs:999-1042` (`clipped_band_finding` builds `compute::toolpath_stats::ClippedBandFinding`); compute: `compute/execute/finish_3d.rs` | module-level cycle `finish <-> compute`; the finding types live in the wrong layer |
| `finish -> trace` and `trace -> finish` | `trace/narrate.rs:131,188,195`, `trace/transform_provenance.rs:503-563` name `RampReachClamp`, `RelinkTotals`, `PencilLinkReport`, `ScallopRuntimeAnnotation` | report types of finish leak into the trace layer |
| `dressup -> finish::pencil` (code) | `dressup/entry_descent.rs:520-541`, `dressup/mod.rs:941` call `plan_entry_ramp`, `tip_contact_radius` | a generic dressup depends on a finishing strategy |
| `stock::sim_triage -> finish::pencil` | `stock/sim_triage.rs:1008` (`ENTRY_LOAD_MIN_PEAK_MM = ENTRY_RAMP_MIN_BITE_MM`) | a sim-triage threshold derives from a pencil constant |
| `metrology -> finish` | `metrology/costing.rs:183-198`, `metrology/floor.rs:249` | metrology reads `relink_fragments` and `scallop_math` |
| `maps -> finish` | `maps/tier_islands.rs:100,1328`, `maps/finish_surface_cache.rs:140` | planner helpers (`label_components`, `morphological_close`) are generic mask code in `finish_planner.rs:713-798` |
| `session -> finish` | `session/project_file.rs:1418`, `session/mutation/config.rs:258`, `session/reach.rs:95`, `session/compute.rs:1021` | session reads pencil/scallop internals |
| `finish/mod.rs` doc claim vs fact | `mod.rs:4-8` says the placement "keeps `surface` free of an upward dependency". No `surface` code imports `finish` (only doc links). True for code. Doc links exist at `surface/rest_field.rs:133,318,342,1611,2011,2037` | doc links to renamed or private items can rot (see 7) |

No edge to `viz`, `cli` or `mcp` crates. Those crates import finish types (section 2).

## 4. Traits

| Trait | Defined | Implementors / use |
|---|---|---|
| `ComposedSurfaceOp` (assoc types `Classified, Fragment, Memo: Default, Annotation, Report`; 6 required fns, 1 default `take_produced`) | `composed.rs:57` (`pub(crate)`) | `UnifiedFinishOp` (`unified_finish.rs:1798`), `SteepShallowOp` (`steep_shallow.rs:672`). The runner side trait `ComposedAnnotate` (`compute/execute/composed.rs:144,175`) is implemented for the same two types. Pencil does not implement it (`composed.rs:8-11`). |
| none else | | The area defines no other trait. |

Traits from elsewhere:

| Trait | Used how |
|---|---|
| `crate::compute::spans::RuntimeLabel` | implemented for `ScallopRuntimeAnnotation` (`scallop.rs:1037`), `PencilRuntimeAnnotation` (`pencil.rs:501`), `SpiralFinishRuntimeAnnotation` (`spiral_finish.rs:121`), `RampFinishRuntimeAnnotation` (`ramp_finish.rs:480`). Radial and horizontal have no annotation type. |
| `MillingCutter` | taken as `&dyn MillingCutter` everywhere (`rest_detector.rs:50`, `surface_link.rs`); no generics. |
| `CancelCheck` | taken as `&dyn CancelCheck` (`composed.rs:75`). |
| `Default` | hand-written `impl Default` on 11 params structs (`scallop.rs:143`, `pencil.rs:227`, `steep_shallow.rs:69`, `ramp_finish.rs:228`, `unified_finish.rs:255`, `finish_planner.rs:247`, `horizontal_finish.rs:33`, `radial_finish.rs:33`, ...). |
| `rs_cam_derive::OpParams` | NOT used. Params structs here are plain structs. The `OpParams` schema lives in `compute/operation_configs.rs`. |
| `rs_cam_derive::ParamEnum` | derived on 6 enums only: `ScallopDirection` (`scallop.rs:50`), `PencilDetector` (`pencil.rs:68`), `ClassificationSampler` (`classify_probe.rs:100`), `SpiralDirection` (`spiral_finish.rs:40`), `CutDirection` (`ramp_finish.rs:42`), `ClaimsReference` (`unified_finish.rs` near 408). `CreaseReference` (`unified_finish.rs:358`) derives serde only. |

## 5. Recurring patterns

| Concern | Pattern used here | Example | Grade | Deviations in area |
|---|---|---|---|---|
| Errors and refusals | The only error type is `Cancelled` (`Result<_, Cancelled>`). A refusal is an `Option`/early return plus a counter in the report (`off_surface`, `too_far`, `ceiling_refused`, `hop_too_far`). No `anyhow`. No production `unwrap`/`expect`/`panic!` (all hits are in test tails: `surface_link.rs:2087`, `finish_planner.rs:1663`, `crease_paths.rs:364`). | `composed.rs:75-98`; `pencil/emission.rs:815-830` | TYPES (cancel) / CONVENTION (refusal counters) | Research arm has `CompactSpiralRefusal` enum (`spiral_finish_compact.rs:133`) and `PolarFailure` (`:578`); product code has no refusal enum. `finish_planner.rs:388-401` logs `tracing::warn!` for an empty grid and continues. |
| State mutation | None. Ops are pure functions of (mesh, index, cutter, params, stock refs). They never touch `ProjectSession` or `Command`. The one global mutable is `SURFACE_BUILDS` (`AtomicU64`). | `finish_setup.rs:360` | TYPES (no session access) | `finish_setup.rs` `SURFACE_BUILDS` counter is a global side channel used by a cache sentry (`maps/finish_surface_cache.rs:132`). |
| Parameters and config | Plain `pub struct *Params` with `pub` fields, bare `f64`, hand `impl Default`. Operator config, ranges and serde live in `compute/operation_configs.rs` and are copied field by field into these structs. No validation here. | `steep_shallow.rs:39-83`, `pencil.rs:97-226`, `unified_finish.rs:132-290` | CONVENTION | Defaults duplicated across two files and kept equal by comment only: `unified_finish.rs:262-286` vs `compute/operation_configs.rs:1879,1910,2032`; `pencil.rs:219-243` doc says "never on the production path"; `bitangency_angle: 160.0` in both `pencil.rs:231` and `operation_configs.rs:1519`. Same six fields (`feed_rate, plunge_rate, safe_z, sampling, stock_to_leave, tolerance`) repeated in 8 structs with different defaults: `safe_z` 30.0 (`steep_shallow.rs:77`, `unified_finish.rs:277`) vs 15.0 (`pencil.rs:237`). |
| Units and frames | Bare `f64`. Unit is a name suffix or a doc line. `_mm` in many names (`rest_cell_mm`, `link_hop_distance_mm`, `intra_region_hookup_mm`) but not in `tolerance`, `sampling`, `stepover`, `safe_z`, `overlap_distance`. Angles: degrees in params (`threshold_angle` `steep_shallow.rs:41`, `bitangency_angle`, `slope_from/to` `ramp_finish.rs:60-62`), radians in research (`conformal_spiral.rs:267-297`). One frame: mesh/stock Z-up already in the op frame. No setup or world frame appears in `finish/`; the caller transforms. `stock_to_leave` is a vertical +Z shift (`unified_finish.rs:140-160`). | `finish_setup.rs:618-622` (`SLOPE_FILTER_*_DEG`) | CONVENTION | Mixed suffix style (`_angle` vs `_deg`: `steep_shallow.rs:41`, `finish_planner.rs` `steep_threshold_deg`, `pencil.rs:100`). `cell_mm` and cell sizes as raw f64 with a `CellSource` provenance enum as the only guard (`finish_setup.rs:101-122`). |
| Identifiers | No `ids.rs` newtypes. Regions and moves use `usize` indices: `region_id` = index into `UnifiedFinishReport::region_table` (`unified_finish.rs:889`, `1340`), `move_range: Range<usize>`, chain/offset indices in `PencilRuntimeEvent`. | `unified_finish.rs:756-760` | CONVENTION | `region_id` has two id spaces (node vs ring); disambiguated by `RegionSpanRole` (`unified_finish.rs:1296-1300`). Pencil/ramp events carry bare `usize` index pairs (`ramp_finish.rs:82-95`). |
| Background work, progress, cancellation | `&dyn CancelCheck` threaded through; `check_cancel(cancel)?` between phases and inside loops. `compose` polls before classify and before each region (`composed.rs:136-142`). No progress callback in this area (rg `progress` = 0). Rayon is used inside samplers (`classify_probe.rs`, TileRaster). | `composed.rs:130-155` | TYPES (Result forces `?`) / CONVENTION (poll frequency) | Entry-point combinatorics: `_with_cancel`, `_with_cancel_and_stage`, `_with_resolution` variants (`scallop.rs:1060,1099,1138`; `steep_shallow.rs:566,595`; `ramp_finish.rs:501,531`). Test-only `run_uncancellable` helpers behind `cfg(test)` (`ramp_finish.rs:462-464`, `spiral_finish.rs:105`, `radial_finish.rs:56`, `horizontal_finish.rs:71`, `scallop.rs:1019`). |
| Caching, freshness, invalidation | None inside the area. Surface cache is outside (`maps/finish_surface_cache.rs`) and keys on `FinishResolutionPolicy` (`Hash`). `FinishSurface` carries `resolution`, `sampler`, `cell_source` provenance. | `finish_setup.rs:55,139,300` | TYPES (Hash on policy) | `surface_build_count()` global (`finish_setup.rs:360`) is the only freshness probe. |
| Diagnostics, warnings, messages | Reports (`ScallopReport`, `PencilReport`, `UnifiedFinishReport`, `RelinkTotals`, `ClaimsReport`) are plain structs with `pub` fields and counters. Findings are built here but typed in `compute::toolpath_stats`. `tracing::warn!/debug!` for developer logs (3 `warn!` in `unified_finish/band_kernels.rs:68,377,400`; `surface_link.rs:978`; 3 in `finish_planner.rs`). `FinishBand::label` is "the ONE place a band becomes text" (`finish_planner.rs:114-120`). | `unified_finish.rs:999-1050` | CONVENTION (+ TYPES for `FinishBand::label`) | `CreaseReference::MachinedStock` doc promises a `tracing::warn!` on fallback (`unified_finish.rs:371`) but no `warn!` exists in that file; `ClaimsReference` doc says "reported rather than warned" (`:441`). `STEEP_HALF_LABEL`/`SHALLOW_HALF_LABEL` string consts are matched by sentries (`steep_shallow.rs:548-551`). `CLAIMS_STEPOVER_DEPTH_BASIS: &str` is a prose const (`unified_finish.rs:946`). |
| Serialization and wire types | Params are not serde. Enums that the operator sets derive `serde` + `ParamEnum` with `rename_all = "snake_case"` and are re-exported into `operation_configs` as-is. Report types are not serde (they are copied into `session/diagnostics_types.rs`). | `scallop.rs:46-52`, `unified_finish.rs:349-358` | TYPES (ParamEnum derive) | `CreaseReference` has serde but no `ParamEnum` (`unified_finish.rs:357`). `MonotoneCellTotals` is read by CLI and session (`session/diagnostics_types.rs:94`, `crates/rs_cam_cli/src/project.rs:104`). |
| Numeric tolerances | Mixed. Named consts for domain values (`SLOPE_FILTER_MIN_DEG`, `FLUSH_EPS_MM` `surface_link.rs:357`, `LOOP_CLOSE_EPS_MM` `:514`, `ENTRY_RAMP_*` `pencil/emission.rs:47-73`). Inline literals for epsilons: `1e-6` (about 25 sites), `1e-9`, `1e-10`, `1e-12`, `1e-15`, `1e-18` (`crest_lines.rs:201-639`, `pencil/emission.rs:86,208,235,266,352,534,559,759`, `pencil_dihedral.rs:322,375`, `scallop_math.rs:49,76,90,160`). | `surface_link.rs:357` | NONE (no shared epsilon module) | `MIN_TRIANGLE_AREA_MM2 = 1e-18` and `EPS_VEC = 1e-12` defined twice (`conformal_spiral.rs:311,314`, `direction_field.rs:114,117`). `1e-6` repeated as a local `tol` in 6 test modules (`steep_shallow.rs:1620`, `spiral_finish.rs:757`, `radial_finish.rs:532`, `horizontal_finish.rs:633`, `ramp_finish.rs:1304`). Ad-hoc cleanup tolerances `1e-5, 1e-6` at `scallop.rs:687`. |
| Geometry primitives | `crate::geo::{P2, P3}` (nalgebra `Point2/3<f64>`, `geo.rs:7,9`), `Polygon2`, `RegionSet<'_>`, `Grid2`, `SurfaceHeightmap`, `SlopeMap`. Masks are `Vec<bool>` plus `(rows, cols)` args. | `finish_planner.rs:81-87` | CONVENTION | `Vec<bool>`/`&[bool]` masks with separate `rows, cols` in `finish_planner.rs` (12 sites), `steep_shallow.rs` (11), `scallop_isofield.rs`, `unified_finish.rs` (4): `dilate_grid(grid, rows, cols, r)` (`steep_shallow.rs:91`) vs `Grid2` in `finish_planner.rs:82`. Tuples `(f64,f64)` in `crest_lines.rs` (3), `surface_link.rs` (2), research files (23). |
| Test style and sentries | Inline `mod tests` per file (24 files) plus split test files `unified_finish/tests.rs` (1707), `pencil/tests.rs` (1209), `scallop/tests.rs` (896), `conformal_spiral/tests.rs` (934). Integration sentries in `crates/rs_cam_core/tests/` (82 files read `finish::`), named by finding id (`a_link_between_regions_clears_standing_stock_b16`, `..._g_tierburial`). Heavy ones `#[ignore]`. Research tests need `--features research`. | `unified_finish.rs:2498`; `Cargo.toml:164-198` `required-features` | SENTRY | Test-only items re-exported with `cfg(test)` to avoid dead-code warnings (`pencil.rs:31-33`, `scallop.rs:1019-1020`, `scallop_math.rs:86-88,172`). Test lint allows: `pencil/tests.rs:4` `#![allow(clippy::unwrap_used, clippy::panic)]`; `scallop_math.rs:173`. Every prod `allow` has a `// SAFETY:` line except several: `scallop.rs:284,286,1059,1098,1137`, `pencil/detectors.rs:200`, `conformal_spiral/spiral_build.rs:173`, `surface_link.rs:1375`, `pencil_dihedral.rs:403` (multi-line allow; check). |
| Naming and layout | `snake_case` file per strategy; sub-folder holds the split-out files of one big file (`scallop/`, `pencil/`, `unified_finish/`, `conformal_spiral/`). `*_toolpath_structured_annotated_with_cancel` for the runner-facing entry. `pub use` re-exports keep old paths after splits (`scallop.rs:32-38`, `pencil.rs:29-38`). | `scallop.rs:18-38` | CONVENTION | `scallop/research.rs` is not a research arm (product iso-field lives there; `finish_raster.rs:119` calls it). `scallop.rs` `cfg(test)` fn at `:1020`. `spiral_finish.rs` (product) vs `spiral_finish_compact.rs` (research) vs `conformal_spiral.rs` (research). |
| Feature registration | A strategy is registered outside this folder: `OperationType` variant, `compute/catalog.rs` (16 refs), `compute/catalog/registry.rs` (8), `compute/config.rs`, runner in `compute/execute/finish_3d.rs` or `finish_raster.rs`, plus `feeds/geometry_class.rs`, `tool_load/chipload.rs`, `session/multitool.rs`, `compute/generated_empty.rs`. Here the strategy only exports params + entry point + annotation. Per-op enums: `FinishBand`/`RegionKind`/`RegionStrategy` are matched with exhaustive `match` (`unified_finish.rs:771-800`). | `compute/execute/finish_3d.rs:317` | TYPES for match arms; CONVENTION for the cross-folder list | `RegionStrategy` is derived from `RegionKind` + band by convention (`unified_finish.rs:771-790`). `FinishBand::ALL` array has 3 entries tied to the enum by hand (`finish_planner.rs:107`). |

Additional finish-specific invariants and their enforcement:

| Invariant (CLAUDE.md) | Held by | Grade | Gap |
|---|---|---|---|
| Straight feeds follow the drop-cutter surface (G-TIERBURIAL) | `surface::chord_refine` called at `unified_finish.rs:221-252`, `scallop/ring_generation.rs:12,157`, `surface_link.rs`; sentries `a_scallop_ring_connector_rides_the_surface_g_tierburial`, `fed_chords_ride_the_surface_g_tierburial` | SENTRY | Not applied to pencil links: `pencil/emission.rs:773-775` passes `None` for tolerance, "the link is not refined (G-TIERBURIAL, open)". Also not applied in `horizontal_finish`, `radial_finish`, `spiral_finish`, `ramp_finish`, `steep_shallow` (rg: `chord_refine` appears in 4 files only). |
| Every link under a ceiling takes `shape_under_ceiling` (B16) and ends in `push_link_landing` (B20) | `surface_link.rs:328,280`; callers `unified_finish/region_routing.rs:483`, `unified_finish.rs:2372`; sentries `..._b16`, `..._b20` | SENTRY | Pencil builds its own lifted link (`pencil/emission.rs:509-535`, `plan_link_lift`) and never calls `shape_under_ceiling` (rg: callers only in `surface_link.rs` and `region_routing.rs`). Scallop research path uses `relink_fragments_with_kinds` (`scallop/research.rs:811`). |
| A product rest field comes only from `rest_detector::detect_rest` | `rest_detector.rs:118`; the three callers (`unified_finish.rs:1554`, `pencil/detectors.rs:234`, `compute/execute/dressup_apply.rs:117`); `detect_rest_valleys` is `pub` in `surface/rest_field.rs` | CONVENTION | No visibility barrier: `surface::rest_field::detect_rest_valleys` is still callable from anywhere. Today only `rest_detector.rs:127` calls it (rg), but nothing enforces it. |
| A composed op states barriers only via `order_constraints` | trait method `order_constraints` + `composed_spans` (`composed.rs:104,159-170`) | TYPES (trait shape) | An op can still build `Span` barriers in `label_spans`; no type stops it. |
| Research arms are feature-gated and no product path calls them | `mod.rs:15,19,32`; `Cargo.toml required-features`; sentry `the_research_arms_are_feature_gated_fin01` | TYPES (cfg) + SENTRY | `scallop/research.rs` and the unused `ClassificationSampler` arms are not gated (see 9). |
| Pencil NMS detector stands; no flow-accum spine | doc only (`CLAUDE.md:23`) | DOC | No sentry named. |

## 6. Extension points

| New item | Files touched | Compiler forces |
|---|---|---|
| New finishing strategy (own op) | 1) new `finish/<name>.rs` + `pub mod` in `mod.rs`; 2) params struct with `Default`; 3) entry fn `..._with_cancel` returning `Result<_, Cancelled>`; 4) optional `RuntimeLabel` impl; 5) outside: `OperationType` and `compute/catalog.rs` + `catalog/registry.rs` + `compute/config.rs` + `compute/operation_configs.rs` (config struct, serde defaults, conversion into params) + `compute/execute/finish_3d.rs` runner + `feeds/geometry_class.rs`, `tool_load/chipload.rs`, `session/multitool.rs`, `compute/generated_empty.rs` | exhaustive `match OperationType` sites (about 40 refs in 10 files); NOT forced: the params-to-config field copy (silent default drift), the `finish/CLAUDE.md` file map, a `FEATURE_CATALOG.md` row |
| New composed op (classify, cut, route) | implement `ComposedSurfaceOp` (`composed.rs:57`) in a new file; add `ComposedAnnotate` impl in `compute/execute/composed.rs:144,175`; runner wiring in `compute/execute/finish_3d.rs:164,317` | trait requires all 6 fns and the 5 assoc types; `ComposedAnnotate` is a separate trait so a missing impl fails only where the runner is generic |
| New Unified Finish band | add `FinishBand` variant (`finish_planner.rs:97`), `FinishBand::ALL` (`:107`), `label` (`:117`); `RegionKind`/`RegionStrategy` arms (`unified_finish.rs:741,771`); a `cut_*` kernel in `unified_finish/band_kernels.rs`; `band_rank` in `region_routing.rs`; `decompose` threshold logic; diagnostics adapters | exhaustive matches on `FinishBand` (labels, rank, strategy); NOT forced: `ALL` length (array literal typed `[Self;3]` does force a size edit), `decompose` thresholds |
| New Unified Finish dial | field in `UnifiedFinishParams` (`unified_finish.rs:132`) + `Default` (`:255`) + `compute/operation_configs.rs` field, `#[serde(default = "...")]` fn, conversion; `finish_3d.rs` build site; MCP/viz property panel in `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs`; schema/doc | struct literal sites (every `UnifiedFinishParams { .. }` without `..Default`) fail to compile; NOT forced: the lockstep default in `operation_configs.rs` |
| New pencil detector | add `PencilDetector` variant (`pencil.rs:71`, `ParamEnum` gives the string key), a detector fn in `pencil/detectors.rs`, dispatch in `pencil.rs:618`; viz sites that compare `== PencilDetector::RestDepth` (`toolpath_panel.rs:933`, `commands.rs:2959`, `session/project_file.rs:1418`, `session/mutation/config.rs:258`) | exhaustive match in dispatch; the `==` comparisons are NOT forced and stay silent for a new arm |
| New scallop ring policy variant | enums at `scallop.rs:304-637` (`RingReducer`, `RingSampling`, `StepoverGeometry`, `CurvaturePolicy`, `PolygonReduce`, `RingSource`, `RingSampleBound`, `RingCleanup`) plus `ring_generation.rs`; policy struct `ScallopStepoverPolicy` (`:527`) | exhaustive matches |
| New classification sampler | `ClassificationSampler` variant (`classify_probe.rs:103`), arm in `sample_classification_grid`, `label`, parity sentries (`classification_strategy_m3`) | exhaustive match; `PRODUCTION` const switch is manual |
| New finish resolution mode | `FinishResolutionMode` variant (`finish_setup.rs:55`), `label` and `cell_source` (`:98-122`, both `const fn` match), a constructor on the policy (`:148-191`) | exhaustive match (the doc at `:113-117` states this is the point) |
| New research arm | file in `finish/`, `cfg(feature = "research")` mod in `mod.rs`, `[[test]] required-features` in `Cargo.toml:164-198`, update `the_research_arms_are_feature_gated_fin01` | cfg only; the sentry list is hand-kept |

## 7. CLAUDE.md drift (`finish/CLAUDE.md`, 40 lines)

| Statement | Reality |
|---|---|
| "Research (`--features research`...): `conformal_spiral*`, `direction_field.rs`, `spiral_finish_compact.rs`" (`CLAUDE.md:17`) | Matches `mod.rs:15,19,32`. But `scallop/research.rs` has "research" in its name and is not gated; the file map does not list it. `CLAUDE.md:16` lists `scallop*.rs, scallop/` only as "rings, height formulas, iso-field". |
| "Every straight feed tracks the drop-cutter surface (G-TIERBURIAL)... ring chords and connectors, the raster and waterline bands, a link with a tolerance" (`:22-24`) | Holds for those four. Says nothing about the exceptions: pencil links (`pencil/emission.rs:773-775` "open"), `horizontal_finish`, `radial_finish`, `spiral_finish`, `ramp_finish`, `steep_shallow`. A reader takes "every" as total. |
| "Every finishing link under a ceiling takes `shape_under_ceiling` (B16)" (`:30`) | Pencil links under a ceiling use `plan_link_lift` (`pencil/emission.rs:509-535`) instead. |
| "A product rest field comes only from `rest_detector::detect_rest`" (`:28`) | True today by rg (`rest_detector.rs:127` is the only caller of `detect_rest_valleys` outside `surface/`), but `unified_finish.rs` module doc (line 24-26) still says `detect_rest_valleys` "runs against THIS op's own tip cutter". Stale text. |
| "W6.4... `--lib finish::unified_finish`" sentry (`:38`) | Test module exists (`unified_finish.rs:2498`, `unified_finish/tests.rs`). OK. |
| All named integration sentries (12 files) | All exist in `crates/rs_cam_core/tests/`. `wanaka_decomposes_to_order_ten_regions` exists at `tests/finish_planner_wanaka_decompose.rs:140`. |
| File map: `steep_shallow.rs, horizontal_finish.rs ... — single-strategy finishes` | `steep_shallow.rs` is also a composed op (`SteepShallowOp`, `:624`); listed correctly in Invariants but not in the map. |
| File map omits `finish_planner.rs` role detail, `scallop_isofield.rs`, `scallop_math.rs`, `pencil_dihedral.rs`, `unified_finish/tests.rs` | `scallop*.rs` and `pencil*.rs` globs cover them. `classify_probe.rs` is mentioned only as a name. |
| `finish_setup.rs` module doc (`finish_setup.rs:1-3`) says it holds "the Z-ladder used to walk from a top Z down" | The file has no `z_ladder`; it lives in `ops/depth.rs` (`ops/depth.rs:52` const; `steep_shallow.rs:181`). `finish_setup.rs:15-33` still links `[`z_ladder`]` and "Every finish-setup piece is single-sourced here ... Z ladder". |
| `ops/waterline.rs:78` doc link `crate::finish::finish_setup::Z_LADDER_DEFAULT_EPSILON` | The const is `ops/depth.rs:52`. Broken link in a neighbouring folder. |
| `unified_finish.rs:371` doc: fallback to `SelfProbe` "with a `tracing::warn!`" | No such warn in the file; `ClaimsReference` doc (`:441`) says reported not warned. |
| `unified_finish.rs:~420` doc link `crate::finish::finish_setup::CellSource` | `CellSource` lives in `crate::measurement` (`finish_setup.rs:30` imports it privately). |
| `pencil.rs:227-243` comment: `Default` "is never on the production path" | True, but the same defaults are copied in `compute/operation_configs.rs:1519`; the comment gives no pointer to that second copy. |

## 8. Navigability

| Item | Lines | Issue |
|---|---|---|
| `unified_finish.rs` | 2499 (all prod; tests are in `unified_finish/tests.rs`) | Mixed: dial bundle (`:132`), claims types (`:358-724`), report types (`:889-1300`), span builders (`:1304`), resolution policy fns (`:1369-1391`), orchestrator (`:1427-1766`, about 340 lines), `UnifiedFinishOp` impl (`:1767-2497`). About 100 lines of module doc are a campaign history log (`:1-92`). |
| `conformal_spiral.rs` | 2981 | research; mixed math, mesh parametrisation, spiral build; 1 file + 2 sub files |
| `surface_link.rs` | 2108 (prod 1373) | mixed: `build_surface_link`, `LinkCeiling`, `shape_under_ceiling`, `FinishingLinkStage`, `relink_fragments` (about 540 lines from `:835`), `RelinkReport`; `FragmentKind` |
| `direction_field.rs` | 2062 | research |
| `finish_planner.rs` | 2002 (prod about 1217) | planner plus generic mask morphology (`and_masks_in_place`, `label_components`, `morphological_close`, `:713-798`) used by `maps/tier_islands.rs:100`; SVG debug writer `planned_regions_to_svg` (`:1041`) in prod code |
| `steep_shallow.rs` | 1766 (prod about 1009) | two op shapes in one file (standalone fn + composed op) |
| `ramp_finish.rs` | 1320 | ok |
| `classify_probe.rs` | 1072 | 5 sampler arms; 4 are oracles/candidates |
| Names | | `scallop/research.rs` (product code, see 7); `finish_setup.rs` (surface building + resolution policy + slope constants, no Z ladder); `composed.rs` (generic name for the W6.4 shape); `spiral_finish.rs` vs `spiral_finish_compact.rs` vs `conformal_spiral.rs` |
| Items in the wrong folder | | `classify_probe`, `crest_lines`, `pencil_dihedral`, `direction_field` are surface analysers placed here to avoid `surface -> finish` (`mod.rs:4-8`). `pencil::{plan_entry_ramp, tip_contact_radius, entry_ramp_window_mm, ENTRY_RAMP_*}` serve `dressup/` and `stock/sim_triage.rs` (see 3). `unified_finish::{clipped_band_finding, dropped_band_finding}` build `compute` finding types. `finish_planner` mask helpers are generic geometry. `scallop_math.rs` is read by `metrology/floor.rs` and `session/reach.rs`. |

## 9. Candidate debt leads (UNVERIFIED)

Each line: path:line + one sentence.

1. `unified_finish.rs:262-286` and `compute/operation_configs.rs:1879,1910,2032` - the same defaults are kept equal "in lockstep" by a comment; a drift gives silent divergence (a divergence class the repo already hit, per the comment).
2. `pencil.rs:227-243` and `compute/operation_configs.rs:1519` - second copy of pencil defaults (`bitangency_angle: 160.0`); `safe_z` default differs by struct (15.0 vs 30.0) with no shared source.
3. `steep_shallow.rs:39`, `ramp_finish.rs:56`, `horizontal_finish.rs:17`, `radial_finish.rs:17`, `spiral_finish.rs:51`, `pencil.rs:97`, `scallop.rs:62`, `unified_finish.rs:132` - eight params structs repeat `feed_rate, plunge_rate, safe_z, sampling, stock_to_leave, tolerance` with no shared type; a unit or rename change needs eight edits.
4. `pencil/emission.rs:773-775` - pencil links are not chord-refined ("G-TIERBURIAL, open") and do not use `shape_under_ceiling`; the folder CLAUDE.md says "every".
5. `surface/rest_field.rs` `detect_rest_valleys` is `pub`; the "only `detect_rest`" rule (`CLAUDE.md:28`) rests on convention (candidate: `pub(in crate::finish)` or a `pub(crate)` wrapper).
6. `pencil.rs` `PencilDetector` compared with `==` at `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:933`, `crates/rs_cam_viz/src/app/mcp/commands.rs:2959`, `session/project_file.rs:1418`, `session/mutation/config.rs:258`, `session/mutation/tests.rs:408` - a new detector variant compiles silently past these.
7. `finish_planner.rs:728-798`, `steep_shallow.rs:91` - two mask-morphology implementations (`morphological_close` via distance transform; `dilate_grid` ring loop) on raw `Vec<bool>` + `rows, cols`.
8. `conformal_spiral.rs:311,314` and `direction_field.rs:114,117` - duplicated epsilon consts in two research files.
9. `crest_lines.rs:201-639`, `pencil/emission.rs:86-759`, `scallop_math.rs:49-160` - many inline epsilon literals (`1e-6`, `1e-9`, `1e-12`, `1e-18`, `1e-30`) with no shared names.
10. `classify_probe.rs:103-135` - `DropCutterProbeScratch`, `TriangleRaster`, `VerticalRay` are candidate arms; only `TileRaster` (production) and `DropCutterProbe` (oracle) are used outside tests. Check dead variants.
11. `finish_setup.rs:380-571` - six surface-builder fns (`..._with_cancel`, `..._with_cell_size_and_cancel`, `..._with_policy_and_cancel`, and three classification twins); `build_finish_surface_with_cell_size_and_cancel` has only a test caller (`scallop/tests.rs`).
12. `steep_shallow.rs:566,595`, `ramp_finish.rs:531`, `scallop.rs:1060,1099,1138` - `_with_cancel` / `_with_resolution` / `_and_stage` entry-point families; some have only test or bench callers.
13. `unified_finish.rs:139-160` - `plan_entry_ramp(&run, stock, contact, start.z, true)` at `dressup/entry_descent.rs:541` passes a bare `bool`; `refine_band_chords(..., level: bool)` at `unified_finish.rs:221` is a bool-flag API.
14. `surface_link.rs:35,327,864`, `scallop.rs:1059,1098,1137`, `pencil/emission.rs:503,631,702`, `pencil/detectors.rs:200`, `pencil/chain_paths.rs:267` - `#[allow(clippy::too_many_arguments)]` on 11 fns; some lack a `// SAFETY:` line (`scallop.rs:1059,1098,1137`, `pencil/detectors.rs:200`) contrary to the workspace rule.
15. `unified_finish.rs:321`, `unified_finish.rs:1767-2497` - the op impl and orchestrator (`:1427`) both build the same sequence; `unified_finish_toolpath_with_cancel` may duplicate `compose` flow (compare before the W6.4 claim "THE sequence" is trusted).
16. `scallop.rs:1037`, `pencil.rs:501`, `spiral_finish.rs:121`, `ramp_finish.rs:480` - four near-identical `*RuntimeAnnotation{move_index, event}` structs plus `RuntimeLabel` impls; radial and horizontal have none.
17. `unified_finish.rs:946`, `steep_shallow.rs:548-551` - label text held in `&str` consts that sentries match by string.
18. `compute/` imports of `finish::*` internals (`trace/narrate.rs:188,195`, `session/diagnostics_types.rs:94`) copy report fields by hand; no single report-to-diagnostics adapter type in `finish/`.
19. `finish_setup.rs:360` and `finish_setup.rs` `SURFACE_BUILDS` - a process-global `AtomicU64` used by a cache sentry; parallel tests share it.
20. `unified_finish.rs:1-92` and `finish_setup.rs:1-60`, `classify_probe.rs:1-60` - historical campaign narrative in module docs; stale links (see 7) show the cost.
21. `scallop/research.rs` product-path file named "research"; also `scallop.rs:1019-1020` and `scallop_math.rs:86-88` `cfg(test)` "test doors" - indicates API kept only for tests.
22. `composed.rs:46-55` - `ComposedRest::standing: Box<dyn Fn(P3) -> bool + 'a>` is a boxed closure; one allocation per op and no `Send/Sync` bound (check the runner's threading).
