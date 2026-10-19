# Map: core-compute (`crates/rs_cam_core/src/compute/`)

Evidence baseline HEAD 99b6fb41, working tree in flux. Read-only map; no source edited.
Paths are relative to `crates/rs_cam_core/src/compute/` unless they start with `crates/`.
Grades: TYPES | LINT | SENTRY | DOC | CONVENTION | NONE.
"in flux" = path is in `instruments/in_flux_paths.txt`. In this area only two files are: `validate.rs` and `catalog/tests.rs`
(also in flux nearby: `crates/rs_cam_core/src/session/compute/generation.rs`, `.../session/compute/tests.rs`).

## 1. Purpose and file map

`compute/` is the data model plus the engine glue between a project and a toolpath. It holds (a) the per-operation config
types, the operation registry/catalogue and its parameter schema; (b) the generation dispatch (`execute*`): prologue, family
runners, dressup pipeline, findings; (c) the simulation orchestrator, its prefix memo, stock carry/stock-change/obstacle/shoe
services and collision check; (d) stats, spans, annotations and the alignment-pin algorithms. `ProjectSession` is the only
driver of generation; the viz worker drives only the simulation. ~42.3k lines (about 30k production, rest in-file tests).

| File | Responsibility | ~Lines |
|---|---|---|
| `mod.rs` | module list + flat re-exports (its doc still says "extracted from rs_cam_viz", Phase 1/2/3) | 62 |
| `catalog.rs` | `for_each_op!` X-macro (op list), `OperationType`, `OperationConfig` enum + 7 exhaustive per-variant matches, `OperationParams` trait, `FeedsHints`, `cutting_levels`, `effective_spindle_rpm` | 1480 |
| `catalog/registry.rs` | 24 `REG_*` statics: spec, params, tool constraints, dressup policy, `OpPolicy`, `Capabilities`, `generate` fn | 1259 |
| `catalog/schema.rs` | `ParamTable/ParamDef/ParamRange/ParamType`, `OpParams` trait, `OpPolicy`, `Kinematics`, `ToolConstraintsDef`, `DressupPolicy`, `OpRegistryEntry`, schema DTOs | 605 |
| `catalog/capabilities.rs` | `Capabilities<P>` (22 fields), `RowCapabilities`, `TRANSFORM_*` consts, `RestUse/ItemUse/TerritoryXy/LutRoute/...` enums | 577 |
| `catalog/tests.rs` | registry sentries (partition, serde repr, pinned sets) - in flux | 993 |
| `operation_configs.rs` | 24 `*Config` structs (`#[derive(OpParams)]`), param enums (`ParamEnum`), help-text consts, `impl_operation_params!` macro + 24 invocations | 3302 |
| `config.rs` | `StockSource`, `ComputeStatus`, heights (`HeightsConfig/ResolvedHeights`), `BoundaryConfig`, `RestAnalysisConfig`, `DressupConfig` + wire struct + `FIELD_DEFS` | 1891 |
| `tool_config.rs` | `ToolId`, `ToolType`, `ToolConfig`, geometry normalisation, tool-name vs geometry mismatch parser, size-unit labels | 1937 |
| `stock_config.rs` | `StockConfig`, `ModelKind/ModelUnits`; calls `alignment_pins` | 318 |
| `transform.rs` | `FaceUp`, `ZRotation`, `SetupTransformInfo` (world-to-setup frame) | 637 |
| `cutter.rs` | `build_cutter(&ToolConfig) -> ToolDefinition` | 51 |
| `execute.rs` | `OperationError`, `GenerationFindings`, `ExecutionContext`, `GenerateFn`, `config_guard!`, `execute_operation_annotated` (the dispatch) | 678 |
| `execute/prologue.rs` | cancel, geometry (`GeometryRequirement`), selection view, tool refusal -> `Prepared` | 266 |
| `execute/shared.rs` | span wrappers, `LinkCeilingNeed`, `link_ceiling_for`, `link_after_plan` | 305 |
| `execute/territory.rs` | `Territory`, `TerritoryInputs` (private fields), obstacle pre-clip, `unguarded_stock` | 447 |
| `execute/layered.rs` | Layered runner (`run_layered`, `LayeredOp`, ladder, rest rule) | 811 |
| `execute/height_field.rs` | Height-field runner (`run_height_field`, `HeightFieldOp`, `HeightQuery`) | 526 |
| `execute/composed.rs` | Composed runner (`run_composed`; Unified Finish, Steep+Shallow) | 187 |
| `execute/volumetric.rs`, `rough_3d.rs` | Volumetric runner + 3D Rough adapter | 252 / 223 |
| `execute/curve.rs` | Curve runner (Project Curve) | 187 |
| `execute/standing_material.rs` | per-level standing-material clip (G-RESTLEVELS), `ContourFilterRule`, `column_top` | 800 |
| `execute/clearing_2d.rs` | adapters + `LayeredOp` impls: Zigzag, Trace, Profile, Pocket, Face; 2D Adaptive own loop | 757 |
| `execute/curve_engrave.rs` | adapters: Inlay, InlayPlug, VCarve, Chamfer, ProjectCurve | 530 |
| `execute/finish_3d.rs`, `finish_raster.rs` | adapters: Pencil, UnifiedFinish, SteepShallow, Ramp, Spiral, Radial, Horizontal / DropCutter, Scallop, Waterline | 603 / 485 |
| `execute/drilling.rs` | Drill, AlignmentPinDrill adapters, `build_drill_op_for_config` | 392 |
| `execute/dressup_apply.rs` | `apply_dressups`, `DRESSUP_PIPELINE`, `DressupStage`, `DressupContext`, generic rest-analysis attach | 940 |
| `execute/findings.rs` | `RecordFindings` trait + 20 impls (the one door into `GenerationFindings`) | 510 |
| `execute/tests.rs` + 4 test-only modules named by ticket (`*_g_bottompin`, `project_curve_chaining`, `unified_finish_*`) | in-folder tests | 1688 + 1680 |
| `simulate.rs` | sim request/result types, 7 shared builder decisions, provenance hashing, frame maps, `run_simulation*` (one ~450-line fn), deviations, kinematics cycle time | 3361 |
| `sim_prefix.rs` | S5 prefix memo for `generate_all` fixpoint (`SimMemo`, keys, snapshots) | 1078 |
| `source_stock.rs` | identity of the stock a rest op read (`SourceStock`, `carve_digest`, wire types) | 454 |
| `stock_carry.rs` / `stock_change.rs` / `stock_change_apply.rs` | S0 carry across setups / S2 stock-change record+refusal+digest / S3 apply to dexel stock | 294 / 605 / 331 |
| `obstacle.rs` | K1 obstacle model, `ObstacleSet::resolve`, `ObstacleToolView`, clip walk, `JoinStart` | 1777 |
| `shoe.rs` | H1 dust-shoe gap solver | 258 |
| `collision_check.rs` | holder/shank/obstacle check wrapper | 286 |
| `toolpath_stats.rs` / `stats.rs` | `ToolpathStats` + ~17 finding types + provenance consts / stat computation, `stats_with_findings` | 1933 / 369 |
| `spans.rs` / `annotate.rs` | spans from events, depth runs, intents / per-op semantic-trace annotators (`pub(super)`) | 993 / 928 |
| `generated_empty.rs` | G-ENTRYEMPTY empty-generation gate | 469 |
| `validate.rs` | stale-default validator (4 closed rules) + fixers - in flux | 795 |
| `alignment_pins/` (mod, keying, placement, walls, pattern, hole_sources, setup_notes) | pin type, setup map, keying audit, placer, walls, set edits, hole sources, notes | 2976 |

## 2. Public entry points and outside callers

Counts are `rg` line hits outside `compute/` (src / core tests / viz / cli / mcp). Comment mentions are included where noted.

| Entry point | Defined | Outside callers |
|---|---|---|
| `execute_operation_annotated` (`pub(crate)`) | `execute.rs:593` | `session/compute.rs:1292` (`execute_generation`), `session/compute.rs:1600` (advisor probe), `session/compute/tests.rs:2063`. viz/cli: comments only (`crates/rs_cam_cli/src/job.rs:1031`, `run.rs:8`). TYPES: crate-private. |
| `apply_dressups` | `execute/dressup_apply.rs:471` (`pub`) | `session/compute.rs:855` only in src; 26 hits in core tests; none in viz/cli. |
| `run_simulation`, `run_simulation_with_phase`, `run_simulation_memoized` | `simulate.rs:1312/1324/1912` | src 37 (`session/compute/simulation.rs:15`), tests 171, viz 45 (`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:130` calls `run_simulation_memoized` directly), cli 9, mcp 2. Two builders, one runner. |
| `run_collision_check` | `collision_check.rs:99` | src 2, tests 1, viz 20. |
| `build_cutter` | `cutter.rs:8` | src 44, tests 60, viz 25. Can panic on a bad `ToolConfig` (see sec. 9). |
| `OperationType::ALL`, `.registry_entry()`, `.capabilities()`, `OperationConfig::new_default` | `catalog.rs` | ALL: src 5 / tests 70 / viz 19 / cli 6 / mcp 1. `registry_entry()`: src 3, tests 9, viz 5, cli 4. `.capabilities()`: src 30, tests 76, viz 6. `new_default`: src 26, tests 80, viz 46, cli 4. |
| `validate_stale_defaults`, `validate_one_toolpath`, `apply_stale_default_to_op` | `validate.rs:116/137/182` (in flux) | viz 3 + cli 6; `apply_stale_default_to_op` called from `crates/rs_cam_viz/src/ui/properties/tab_badges.rs:283`. |
| `classify` / `EmptyGenerationInputs` (empty gate) | `generated_empty.rs:275` | src 24, tests 19, viz 3. |
| `ObstacleSet::resolve`, `solve_gap`/`shoe::resolve`, `carry_stock_into_group`, `apply_group_stock_changes` | `obstacle.rs`, `shoe.rs`, `stock_carry.rs:130`, `stock_change_apply.rs:287` | src 4 / 5 / 0 / 0 outside (callers are inside `compute/simulate.rs`); carry has 2 test callers. |
| `stats_with_findings`, `compute_stats_with_spans` | `stats.rs:181/27` | src 2, tests 10, viz 1. |
| `operation_spans` (re-exported `mod.rs:59`) | `spans.rs:18` | 0 outside the file. Re-export has no external user. |
| `compute::*` path overall | | src 133 files / 838 lines, tests 313 files / 1193 lines, viz 191 files / 760 lines (most of the viz hits are viz's own `compute::worker`), cli 8 files, mcp 1 file, mcp_proxy 0. Top submodules by use: catalog 213, config 199, tool_config 170, simulate 108, operation_configs 108, toolpath_stats 104, execute 85, transform 70, stock_config 69, alignment_pins 63. |

## 3. Dependencies and layering

Imports from other core folders (count of `crate::<x>` uses inside compute, tests excluded): compute (self) 326, finish 173, ops 97, trace 64,
geo 61, toolpath 51, tool 39, stock 34, dressup 32, polygon 31, dexel_stock 30, measurement 27, machine 23, feeds 23, geometry 22,
ids 20, adaptive3d 20, surface 18, session 17, mesh 16, selection 15, interrupt 15, budget 15, adaptive 15, material 8, maps 8,
tool_load 1, gcode 1 (test), export 1 (test), diagnostics (tests only).

Layering inversions and cycles:
- `validate.rs:44` imports `crate::session::{ProjectSession, SessionError, ToolpathConfig}`, takes `&ProjectSession`, and `validate.rs:164-174` mutates it via the crate-private `toolpath_configs_mut` (declared `crates/rs_cam_core/src/session/mod.rs:1809`). `compute` should be below `session`. in flux.
- `simulate.rs:180,201` (`group_obstacles`, `group_stock_changes`) and `obstacle.rs:326,447,477` take `crate::session::SetupData` / `LoadedModel`. Session data types leak into compute services.
- Lower layers import compute: `finish/*`, `adaptive/path.rs:3157`, `adaptive3d/path.rs:1692` implement `crate::compute::spans::RuntimeLabel`; `finish/*` and `ops/*` also read `compute::operation_configs` params (`FinishPlannerParams` direction goes the other way too, `operation_configs.rs:5`). So `compute <-> finish/ops/adaptive*` is a cycle in module terms (legal in one crate, not enforced).
- `catalog/registry.rs:11` imports `compute::execute::{LinkCeilingNeed, generate_*}` while `execute` imports `catalog` everywhere. Intentional (registry row holds the `generate` fn).
- `compute::tool_config` pulls `crate::tool::size_units` and defines UI-label strings (`diameter_field_label`, `size_label`, `caution_line`): presentation in the model.
- `compute/execute/tests.rs:99,1233` import `crate::diagnostics::adapters::*` (tests only). `validate.rs:46` imports `tool_load::plunge_stress` (compute -> tool_load, 1 edge).
- `rs_cam_viz` re-implements nothing of dispatch (CMP-11 holds: `generate_via_core` is gone); but viz calls the sim runner directly (sec. 2).

## 4. Traits

Defined here:

| Trait | Defined | Implementors | Enforcement |
|---|---|---|---|
| `OperationParams` | `catalog.rs:673` | the 24 `*Config`, all by `impl_operation_params!` (`operation_configs.rs:2775`, invocations 2864-3051). Zero hand impls. | TYPES (macro requires `plunge_rate:` and `depth_semantics:`), defaults silently `None`/`false` for the rest |
| `OpParams` (`const PARAMS`) | `catalog/schema.rs:133` | 24 configs via `#[derive(OpParams)]` (crate `rs_cam_derive`) | TYPES (a field type without `ParamType` does not compile) |
| `ParamType` | `catalog/schema.rs:144` | `f64,u32,usize,bool,Vec<[f64;2]>,Option<T>,ToolId,ModelId`; enums via `#[derive(ParamEnum)]` | TYPES |
| `RecordFindings` | `execute/findings.rs:28` | ~20 report/finding types (`:96-401`) + `Option<T>`, `Vec<T>`, `()` | CONVENTION (an adapter may bypass `ctx.record` by `findings.borrow_mut()`, `pub` field) |
| `LayeredOp` | `execute/layered.rs:228` | `PocketContour, ZigzagRaster, TracePass, ProfilePass, FacePass` (`clearing_2d.rs:182-561`), `WaterlinePass` (`finish_raster.rs:372`), `ChamferPass` (`curve_engrave.rs:261`) | TYPES (required methods) |
| `LayerInput` | `layered.rs:144` | `Polygon2`, `MeshInput` | TYPES |
| `HeightFieldOp` | `execute/height_field.rs:217` | `ScallopPass, DropCutterPass` (`finish_raster.rs:63,178`), `RampPass, SpiralPass, RadialPass, HorizontalPass` (`finish_3d.rs:357-565`), `VCarvePass` (`curve_engrave.rs:176`) | TYPES |
| `AnalyticField` | `height_field.rs:117` | `VCarveField` (`curve_engrave.rs:162`) | TYPES |
| `VolumetricOp` | `volumetric.rs:109` | `Adaptive3dRough` (`rough_3d.rs:162`) | TYPES (+ `the_declaration_matches_the_row` SENTRY) |
| `CurveOp` | `curve.rs:60` | `ProjectCurveOp` (`curve_engrave.rs:395`) | TYPES |
| `ComposedAnnotate: ComposedSurfaceOp` | `composed.rs:71` | `UnifiedFinishOp`, `SteepShallowOp` (`composed.rs:144,175`); supertrait impls in `finish/unified_finish.rs:1798`, `finish/steep_shallow.rs:672` | TYPES |
| `RuntimeLabel` (`pub(crate)`) | `spans.rs:28` | `Adaptive*, Adaptive3d*, Scallop*, SpiralFinish*, RampFinish*, Pencil*RuntimeAnnotation` (in `adaptive/`, `adaptive3d/`, `finish/`) | TYPES |

Not on a shape (own loop, no trait): 2D Adaptive (`clearing_2d.rs:643`), Drill, AlignmentPinDrill, Pencil, Inlay, InlayPlug (plain `fn generate_*`).
Std traits: `From<StopReason>`, `From<Cancelled>`, `From<String>` for `OperationError`; `From<Cancelled>`/`From<StopReason>` for `SimulationError`; `CollisionCheckError: From<Cancelled>`; `Display + Error` on each error enum. Serde `From/Into` wire pair on `DressupConfig` (`config.rs:896`). Deliberately not used: no `Default`-via-trait registry, no `dyn` op trait (dispatch is a fn pointer in the registry row, `GenerateFn` `execute.rs:497`); `OperationParams` is used as `&dyn` only via `as_params()`.

## 5. Recurring-pattern table

| Concern | Pattern used here | Example path:line | Enforcement | Deviations inside the area |
|---|---|---|---|---|
| Errors and refusals | One enum per service: `OperationError{MissingGeometry,InvalidTool,Cancelled,OverBudget,Selection,Other(String)}`, `SimulationError`, `CollisionCheckError`, `StockChangeRefusal`, `ObstacleRefusal`, `PinPlacementError`, `GeneratedEmptyRefusal`. Typed refusal text built in `Display`. `?` converts `Cancelled`/`StopReason`. Production `unwrap/expect/panic/indexing` denied by workspace lint. | `execute.rs:28`, `simulate.rs:832`, `execute.rs:231-262` | LINT (`Cargo.toml:36-42`) for panics; TYPES for variants; payloads are `String` | `Other(String)` used for registry-mismatch (`execute.rs:510`, `prologue.rs:~180`) and session cases (`session/compute/generation.rs:772`); `SimulationError::ObstacleRefused(String)` (`simulate.rs:~855`) and `StockChangeRefused{reason:String}` flatten typed refusals to prose; `hash_operation_config` swallows serde error with `unwrap_or_default` (`simulate.rs:939`); `ComputeStatus::Error(String)` (`config.rs:~100`). |
| State mutation | Compute holds config types and pure services. Mutation is by session `Command`. Public setters on `OperationConfig` (`set_feed_rate`, `set_stepover`...) return `bool` when the field is absent (N5). | `catalog.rs:673-760`, `catalog.rs:1019` (`selection_slot` is `pub(crate)`) | TYPES (visibility) + SENTRY (`tests/hatches_are_crate_private_wp7.rs`, `setters_have_rows_wp15a.rs`) | `validate.rs:164` `apply_stale_default_fix(&mut ProjectSession)` writes through `toolpath_configs_mut` (bypasses `Command::apply`, no stale/invalidate); `validate.rs:182` is `pub` and viz writes an `OperationConfig` + `feeds_provenance` in place (`tab_badges.rs:283`); `ToolConfig::normalize_geometry(&mut self)` (`tool_config.rs:438`) and `StockConfig::update_from_bbox` (`stock_config.rs:138`) are pub `&mut` mutators; `OperationConfig::set_feed_rate` etc. are `pub` (callable from viz without a Command). |
| Parameters and config | `#[derive(OpParams)]` on each `*Config`; `#[param(help, range, optional, driver_set, frame_dependent, skip)]`; `Default` hand-written per config; help text consts shared. Setter and GUI read one `ParamRange`. | `operation_configs.rs:213-255`, `catalog/schema.rs:318-343` | TYPES (derive) + SENTRY (`catalog/tests.rs` W5 partition, default-in-range) | `ParamRange` is optional (`None` = unstated; most feeds/rpm params have none, `schema.rs:~225`); project TOML is not range-validated (doc `schema.rs:~215`); no `deny_unknown_fields` anywhere in the area (typos in a project file are ignored silently); `skip` has no user ("No config has one today" `schema.rs:97`); per-config `fn default_*()` serde defaults (`operation_configs.rs:1211-1298`, `:2119-2203`) must be kept equal to `Default` by hand. |
| Units and frames | mm everywhere (`*_mm` suffix often, not always); degrees for angles (`ramp_angle`, `included_angle`, `slope_from`); Z values bare `f64`; "setup frame" vs "stock-relative global" vs "world" documented in prose. `SizeUnits` is a display-only unit. | `config.rs:162` (`SAFE_Z_CLEARANCE_MM`), `simulate.rs:420-445` (frame contract in doc), `tool_config.rs:~187` | DOC / CONVENTION | No frame or unit newtype. `ExecutionContext` says "already in emission frame" (`execute.rs:355`); `HeightMode::Manual(f64)` is an absolute Z with no frame tag (`config.rs:253`); mixed suffixes `entry_clearance_mm` / `helix_pitch` / `tolerance` / `stepover`; angle params named `*_deg`, `angle`, `ramp_angle`, `taper_half_angle` with no unit in type; `ParamDef::frame_dependent` flag is the only machine-readable frame marker. |
| Identifiers | `ids.rs` newtypes `ToolpathId, ModelId, SetupId, ObstacleId, StockChangeId` (`#[serde(transparent)]`); indexes stay `usize` on purpose. | `crates/rs_cam_core/src/ids.rs:24-49` | TYPES (tuple structs with `pub` field, so no private ctor) | `ToolId` lives in `tool_config.rs:8`, not `ids.rs`. Bare `usize` for setup id: `stock_change.rs:437`, `stock_change_apply.rs:50`, `simulate.rs:857`, `source_stock.rs:81,295`; for toolpath id: `alignment_pins/hole_sources.rs:41,83`; raw tool id `validate.rs:361`; `ParamType` maps `ToolId`/`ModelId` to `"usize"` (`schema.rs:187-198`). |
| Background work, progress, cancellation | Cancel = `&AtomicBool` read with `SeqCst`; adapters get `ctx.cancel_fn()` / `ctx.is_cancelled()`; kernels return `Result<_, Cancelled>`. Progress = `FnMut(&str)` phase label. Memory budget stops via `Cancelled` then caller maps `StopReason`. rayon only in deviation pass. | `execute.rs:427-447`, `simulate.rs:1324`, `collision_check.rs:99` | SENTRY (`cancellable_families_honour_a_preset_cancel_flag`, `every_generator_polls`, `execute/tests.rs`) | Each of `collision_check.rs:~100`, `simulate.rs`, `layered.rs` re-writes `\|\| cancel.load(Ordering::SeqCst)` instead of using `cancel_fn` (which is `pub(crate)` on the context only); phase labels are free `&str` (`simulate.rs:1913`, no enum); two cancel vocabularies (`AtomicBool` vs `interrupt::CancelCheck`, `clearing_2d.rs:~20`). |
| Caching, freshness, invalidation | Content digests and `Weak<Arc>` identity: `SourceStock` (stale rest snapshot), `SimMemo` prefix key, `StockSnapshotStamp`, `hash_operation_config`, `hash_toolpath`, `effect_digest`. Key closure documented in a table. | `source_stock.rs:149,184`, `sim_prefix.rs:40-75`, `simulate.rs:937-1000` | SENTRY (`sim_prefix_memo_s5`, `stock_changes_stale_*_s2`) + DOC | Four hash recipes: FNV (`source_stock.rs:149`, `toolpath_stats.rs` stamp), `DefaultHasher` (`simulate.rs:912`, `sim_prefix.rs:380`), serde-JSON bytes (`simulate.rs:939`). `hash_toolpath` (`simulate.rs:950`) hashes feeds, `carve_digest` ignores them (intended, but two near-identical walkers). Duplicate helpers `hash_f64` (`simulate.rs:921`) vs `hash_f64_bits` (`sim_prefix.rs:380`). `DefaultHasher` is not stable across Rust releases, yet digests are stored in `SimulationProvenance` and compared at load (`simulate.rs:925-935`). |
| Diagnostics, warnings, user text | Findings, not logs: adapters call `ctx.record(&report)`; `GenerationFindings` -> `ToolpathStats` via `stats_with_findings` (exhaustive destructure, no `..`); diagnostics adapters live in `crate::diagnostics`. Refusal texts live on the registry (`tool_refusal`). | `execute/findings.rs:28`, `stats.rs:181-290` | TYPES (exhaustive destructure) | `tracing::warn!` still used in places (`execute.rs` doc admits the history); `ComputeStatus`/`AwaitingPriorStock.message` is a built sentence (`config.rs:70`); `PROJECT_CURVE_POINT_CAUTION` string const in `operation_configs.rs:2586`; UI labels in model types (`ToolConfig::size_label`, `HeightReference::label`). |
| Serialization and wire | serde derive on configs; `OperationConfig` is `#[serde(tag="kind",content="params")]`; `DressupConfig` converts to a flat bool+value wire struct; `SourceStockWire`; `ParamHint`/`OperationSchema` DTOs; `kind_str()` pinned to serde repr. | `catalog.rs:748`, `config.rs:896-1040`, `source_stock.rs:275-312` | SENTRY (`operation_type_serde_repr_pinned`, `dressup_field_names_are_published`) | `AwaitingPriorStock` is `Serialize` only (`config.rs:54`); `params_value_including_nulls` rebuilds JSON by hand (`catalog.rs:~1235`); type strings `"f64"`, `"option<f64>"`, `"enum:a\|b"`, `"vec<..>"` are stringly (`schema.rs:144-198`, prefix test `catalog.rs:~1245`); dressup keys are `&str` in `owner_of_value_field`/`is_dressup_enabled` (`config.rs:1190,1220`); `FIELD_DEFS` hand table next to struct. |
| Numeric tolerances | Local named consts per module, some documented; many bare `1e-9`/`1e-6` literals. | `execute/layered.rs:111` (`FINAL_Z_EPS_MM`), `obstacle.rs:823-838` (`Z_EPS_MM`,`XY_EPS_MM`,`T_EPS`), `config.rs:375` (`ZERO_DEPTH_EPSILON`), `alignment_pins/mod.rs:99` | CONVENTION | No shared epsilon module in the crate (each folder has its own: `selection/key.rs:24`, `ops/waterline.rs:83,118`, `surface/flow_accum.rs:32`, ...). In compute: 26 bare `1e-` literals in non-test code, concentrated in `stock_config.rs` (10), `obstacle.rs` (10), `alignment_pins/walls.rs` (8), `validate.rs` (6). Name-vs-geometry rules `NAME_MATCH_REL_TOL=0.10` (`tool_config.rs:928`). |
| Geometry primitives | nalgebra aliases `P2/P3/V3/BoundingBox3` (`crate::geo`), `Polygon2` (`crate::polygon`), `TriangleMesh/SpatialIndex`, `TriDexelStock`. | `execute.rs:19-20`, `territory.rs:30` | CONVENTION | Tuples and arrays as points: `(f64,f64)` 59 uses, 29 in `alignment_pins/walls.rs`; `[f64;2]` for holes (`execute/drilling.rs` 6) and `Vec<[f64;2]>` as a `ParamType` (`schema.rs:181`); `AlignmentPin{x,y,diameter}` loose fields (`alignment_pins/mod.rs:57`). |
| Tests and sentries | Tests colocated (`mod tests`, `#[allow(clippy::unwrap_used..)]` on the test mod) plus 499 integration test files in `crates/rs_cam_core/tests/` named `<claim>_<ticket>.rs`; table-driven loops over `OperationType::ALL`; golden `RS_CAM_BLESS_OPGOLDEN`. ~220 `#[test]` in-area. | `catalog/tests.rs`, `execute/tests.rs`, `crates/rs_cam_core/tests/every_op_conforms_w4.rs:304` | SENTRY | Test-only modules live beside production files under `execute/` with ticket names (`execute.rs:638-678`); `execute/tests.rs` 1688 lines, `catalog/tests.rs` 993; the folder CLAUDE.md names every sentry and all exist (verified). |
| Naming and layout | Folder per shape runner; files by op family; ticket tags (`W6.3`, `G-LINKCEIL`) in docs and test names; `*_configs.rs`, `*_config.rs`. | `execute/` | CONVENTION | `config.rs` vs `*_config.rs` vs `operation_configs.rs` vs `catalog.rs` (4 places say "config"); `OperationFamily` exists in `catalog.rs:42` AND `feeds::OperationFamily` (aliased `FeedsOperationFamily`) AND `UiOperationFamily`; `needs_both` (`catalog.rs:~1119`) means `ItemsAndMesh` after rename (variant was `Both`); `generated_empty.rs` is a gate not a data file. |
| Registering a new feature | One X-macro row + one registry static + one config struct + hand-written exhaustive matches (sec. 6). Policies are row fields, not `matches!` lists (CMP-07). | `catalog.rs:358-388` (`for_each_op!`), `catalog/registry.rs:36` | TYPES for the matches; SENTRY for the hand lists | `ALL_2D`/`ALL_3D` hand lists (`catalog.rs:470-495`) held by `operation_partitions_cover_all_variants_once`; `OpPolicy` constants make "ordinary" the default a new op can pick without thinking (`schema.rs:492-522`); `relink_dial` / `clearing_strategy_dial` are `Option<&'static str>` param names (`capabilities.rs:81,111`), held by `every_op_states_its_capabilities_w2_4`. |

## 6. Extension points

**New operation** (e.g. "Foo"). Files and arms, with the compiler's role:

| Step | File | Forced by compiler? |
|---|---|---|
| 1. Config struct `FooConfig` + `Default` + `#[derive(OpParams)]` + `#[serde]` shape | `operation_configs.rs` | no (but step 3 will not compile without it) |
| 2. `impl_operation_params!(FooConfig {..})` | `operation_configs.rs:~3051` | yes (`OperationConfig::as_params` needs `FooConfig: OperationParams`) |
| 3. Row in `for_each_op!` (variant, config, category, kind token) | `catalog.rs:358-388` | n/a: this IS the list; generates `OperationType`, `ALL`, `category`, `name`, `kind_str`, `op_type`, `new_default`, `as_params(_mut)` |
| 4. Variant in `enum OperationConfig` (hand-written, serde) | `catalog.rs:748` | yes (`op_type()` match) |
| 5. `registry_entry` arm + `REG_FOO` static (spec, params, tool constraints, dressup policy, policy, capabilities, `generate`) and its `use` lists | `catalog.rs:~507-533`, `catalog/registry.rs` | yes (exhaustive match; no `Option` generate) |
| 6. `ALL_2D` or `ALL_3D` entry | `catalog.rs:470-495` | no; SENTRY `operation_partitions_cover_all_variants_once` |
| 7. Exhaustive matches on `OperationConfig`: `capabilities` (`:~820`), `surface_mask` (`:~910`), `stock_to_leave_dial` (`:~950`), `selection` (`:~995`), `selection_slot` (`:~1019`), `default_selection` (`:~1050`) | `catalog.rs` | yes (no wildcard) |
| 8. Adapter `generate_foo` (pick a shape: Layered, HeightField, Composed, Volumetric, Curve, or plain) with `config_guard!` | `execute/<family>.rs`; re-export in `execute.rs:~540-570` | partly: fn pointer must match `GenerateFn`; the adapter's `op_type()`/`config_guard!` variant is runtime (`execute.rs:510`) |
| 9. Annotator (`annotate_foo`) and spans rule (`SpanRule`) | `annotate.rs`, runner | no: "wrong or empty spans simulate fine" (`execute.rs:~480`); SENTRY `every_op_conforms_w4` |
| 10. Cancel poll in the kernel | kernel | no; SENTRY `every_generator_polls..` (asserts it covers `ALL`) |
| 11. Finding writer if new report | `execute/findings.rs`, `execute.rs:83`, `toolpath_stats.rs`, `stats.rs:181` | yes for `stats_with_findings` (exhaustive destructure); `RecordFindings` impl is convention |
| 12. Golden + fixtures | `crates/rs_cam_core/tests/every_op_keeps_its_toolpath_g_opgolden.rs`, `tests/common/op_fixtures.rs`, `tests/fixtures/op_golden_g_opgolden.toml` | no; SENTRY |
| 13. Outside: viz `OP_UI_ROWS` (`crates/rs_cam_viz/src/ui/properties/operations/registry.rs:~449`, held against `ALL`), per-op editor `ed_foo` (param names as strings, `surface_3d.rs:396+`), `geometry_class.rs:154`, session mutation/config and project_file special cases (`session/mutation/config.rs:257`, `session/project_file.rs:1417`) | viz, session | viz row: SENTRY; others: no |

Roughly 13 touch points, 8 compiler-forced; ordering/spans/cancel/golden are sentry-held.

**New parameter on an existing op.** Add a field to `*Config` (with `#[param(..)]`, serde default fn if optional, `Default`); the derive publishes it; if it is a feeds-style accessor add a clause to `impl_operation_params!`. Compiler forces: nothing for use. Add a viz editor row by param name string (`crates/rs_cam_viz/src/ui/properties/operations/*.rs`, stringly). SENTRY: `catalog` W5 partition, default-in-range, `set_param_refuses_absent_field_n5`.

**New capability field.** Add to `Capabilities` (`capabilities.rs:42-136`) and to every `REG_*` literal (24 edits, struct literal without `..` so the compiler forces each), then give consumers `OperationConfig::capabilities()`.

**New finding.** `GenerationFindings` field (`execute.rs:83`), `ToolpathStats` field + finding type (`toolpath_stats.rs`), `RecordFindings` impl (`findings.rs`), `stats_with_findings` (3 places, compiler-forced), diagnostics adapter (outside, `crate::diagnostics::adapters::from_generation`).

**New dressup.** `DressupConfig` field + `DressupConfigWire` field + both `From` impls + `FIELD_DEFS` row + `owner_of_value_field`/`is_dressup_enabled` string arms + `DressupStage` const + `DRESSUP_PIPELINE` entry + a hand-ordered block in `apply_dressups` (`dressup_apply.rs:471`). Compiler forces the struct literals only; `DRESSUP_PIPELINE` order and arms are SENTRY (`tests/dressup_span_invariants.rs`, `dressup_field_names_are_published`).

**New simulation shared decision.** Put it in `simulate.rs` beside the seven named in the folder CLAUDE.md; nothing forces both builders (session, viz worker) to call it (DOC/CONVENTION).

**New stale-default rule.** Library is closed at four by doc (`validate.rs:20-30`); adding means a `StaleDefaultRule` variant (compiler forces `id`, `title`, `apply_stale_default_to_op`) plus a `check_*` fn and a call in `validate_one_toolpath` (not forced).

## 7. CLAUDE.md drift (`compute/CLAUDE.md`, 40 lines, but several lines are 1500-2500 chars)

| Statement | Reality |
|---|---|
| Files list names `execute/{prologue,findings,standing_material,layered,territory,height_field,composed,volumetric,rough_3d,curve}.rs`, `catalog/capabilities.rs` | Omitted: `execute/shared.rs`, `clearing_2d.rs`, `curve_engrave.rs`, `dressup_apply.rs`, `drilling.rs`, `finish_3d.rs`, `finish_raster.rs`, `catalog/registry.rs`, `catalog/schema.rs`; plus 4 test-only modules under `execute/`. `execute/shared.rs::link_ceiling_for` is cited in a sentry line but `shared.rs` is not in the file list. |
| "A new operation needs a catalogue row and a config variant that derives `OpParams`" | True but incomplete: also the `for_each_op!` row, `registry_entry` arm, `ALL_2D/3D`, six hand matches, `impl_operation_params!`, golden. See sec. 6. |
| "A dressup's parameters live INSIDE its `Option`... with no value field beside its bool" | `feed_optimization`+`feed_max_rate`+`feed_ramp_rate` and `optimize_rapid_order` keep the flat shape (`config.rs:~922-927`; the exception is explained only in the code comment). |
| "`SelectionInputs` is private to `prologue.rs` (C3)" | `SelectionInputs` is `pub` and re-exported (`execute.rs:~557`); `ExecutionContext.selection` is a `pub` field (`execute.rs:~341`). Only `.resolved` read discipline is by convention. |
| "`TerritoryInputs` fields are private to `territory.rs`" | True (TYPES). But `ExecutionContext.territory_inputs` is `pub`. |
| "Seven SHARED decisions in `simulate.rs`" | All seven exist (`simulate.rs:95,179,200,271,286,303,359`). Nothing enforces that the viz builder uses them. |
| Sentries | Every named sentry exists (29 integration test files and 8 `--lib` tests checked by file/fn name; behaviour not run). |
| In-code docs (not CLAUDE.md) that drifted: `mod.rs:1-6` "Phase 1..3 extracted from rs_cam_viz"; `execute.rs:~420-426` the `new` doc is glued onto `cancel_fn`, so `new` (`:453`) has no doc; `execute.rs` doc says findings are a `Cell` (it is `RefCell`) and mentions an `ExecutionContext` field `boundary_regions` that does not exist; `toolpath_stats.rs:1-8` says "the nine finding types" (there are ~17); `crates/rs_cam_viz/src/compute/worker/gen_parity_p0_tests.rs:32` cites `catalog.rs:2749` (file has 1480 lines). |

## 8. Navigability

| Item | Note |
|---|---|
| `operation_configs.rs` 3302 lines (about 3050 production) | 24 configs + enums + help consts + macro; four configs are 250-290 lines each (`UnifiedFinishConfig` 1722-2008). Over the 1500 bar. |
| `simulate.rs` 3361 (about 2700 production) | request/result types, provenance hashing, frame transforms, orchestrator, deviation pass, cycle-time. `run_simulation_memoized` is one function of ~450 lines (`:1912-2360`). Over the bar. |
| `config.rs` 1891, `tool_config.rs` 1937, `toolpath_stats.rs` 1933, `obstacle.rs` 1777, `catalog.rs` 1480, `execute/tests.rs` 1688 | `config.rs` mixes status, heights, boundary, rest analysis and dressups (5 topics). `toolpath_stats.rs` mixes the carrier, 17 finding types with logic, and provenance consts. `tool_config.rs` mixes the model with a name-parsing heuristic (`:940-1280`) and unit labels. `obstacle.rs` mixes model, resolver, tool view, clip walk. |
| Wrong folder / layer | `validate.rs` (session-level logic, imports `ProjectSession`); `tool_config.rs` name parser and labels (presentation); `alignment_pins/` and `shoe.rs` are domain services, fine, but `stock_config.rs` calls into pins; `ToolId` should sit with `ids.rs`. |
| Misleading names | `compute/config.rs` is not "the" config; `GeneratedToolpath = AnnotatedToolpath` alias hides the type; `needs_both` (`catalog.rs`) after `Both` -> `ItemsAndMesh`; `GeometryRequirement::Items` doc says it replaced `Polygons`/`DrillTargets`; test-only modules named by ticket sit among production files in `execute/`; `unguarded_stock` name hides "raw". |
| Doc density | `ExecutionContext` carries a 40-line history paragraph (`execute.rs:~285-317`) that enumerates families by name; hard to keep true. |

## 9. Candidate debt leads (UNVERIFIED)

1. `validate.rs:164-174` `apply_stale_default_fix` mutates the session through `toolpath_configs_mut`, skipping `Command::apply` and any staling; only tests call it. in flux.
2. `validate.rs:182` and `crates/rs_cam_viz/src/ui/properties/tab_badges.rs:283`: GUI mutates an `OperationConfig` in place on a transient entry then re-syncs; a second mutation path.
3. `validate.rs:44` compute depends on `crate::session` (layer inversion); `simulate.rs:180,201`, `obstacle.rs:326,447,477` take `session::SetupData/LoadedModel`.
4. `build_cutter` (`cutter.rs:8`) can panic on a `ToolConfig` whose type-specific geometry is zero (cutter constructors `assert!`); protection is "callers must call `missing_defining_geometry`" (`tool_config.rs:457`), pure convention; the config has all type fields at once instead of an enum per tool.
5. `ToolConfig` has two radii: `corner_radius_mm` (flat end mill, `tool_config.rs:171`) and `corner_radius` (bull nose, `:173`); `list_tools` once published `max()` of both (`tool_config.rs:~433`). Dead-field class bug family; `normalize_geometry` exists to clean up after it.
6. `ToolId` (`tool_config.rs:8`) is outside `ids.rs`; bare `usize` setup/toolpath ids at `stock_change.rs:437`, `stock_change_apply.rs:50`, `simulate.rs:857`, `source_stock.rs:81,295`, `alignment_pins/hole_sources.rs:41,83`, `validate.rs:361`.
7. Production `#[allow(clippy::too_many_arguments)]` without a `// SAFETY:` line: `simulate.rs:1498`, `simulate.rs:1575`, `simulate.rs:2533` (comment but no `SAFETY:`), `execute/dressup_apply.rs:58` (comment, no `SAFETY:`). Breaks the workspace rule.
8. Hash recipes duplicated: `hash_toolpath` (`simulate.rs:950`) vs `carve_digest` (`source_stock.rs:149`); `hash_f64` vs `hash_f64_bits` (`sim_prefix.rs:380`); `DefaultHasher` digests persisted in `SimulationProvenance` (`simulate.rs:925-1000`) are compared across loads; `hash_operation_config` uses `unwrap_or_default` on serialisation (`simulate.rs:939`) so an error hashes as the empty config.
9. `config_guard!` (`execute.rs:510`, 24 uses): every adapter re-matches the variant at runtime and returns `OperationError::Other` on mismatch; `HeightFieldOp::op_type()` etc. state the op a second time (`finish_3d.rs:~420`). A typed config per adapter would remove the runtime branch.
10. `Prepared` accessors (`prologue.rs:~130-165`) return `Err(Other)` when an adapter asks for geometry its row does not require; typestate per `GeometryRequirement` would make it a compile error.
11. `catalog.rs`: `selection`, `selection_slot`, `default_selection` repeat the same 12-arm list three times (`:~995-1070`); `surface_mask`, `stock_to_leave_dial` repeat 16-18 negative arms. A list of ops with a selection slot lives in four places (+ registry `item_use`), held by `check9` in `every_op_conforms_w4.rs:1580`.
12. `Capabilities` has 11 `bool` flags and `OperationTransformCapabilities` 9 (`catalog.rs:~100-170`, `capabilities.rs:95-126`): `plans_bands`, `ramp_descent`, `adaptive_woc`, `flat_floor_clearing`, `top_on_stock_box`, `floor_inside_stock`, `honors_pinned_bottom_z`... A bool bag, each added by a "with_*" builder; relations between flags are prose.
13. `relink_dial` and `clearing_strategy_dial` are `Option<&'static str>` param names (`capabilities.rs:81,111`); a renamed field breaks them at runtime (SENTRY only).
14. No `deny_unknown_fields` in any serde type of the area (0 hits): a misspelt project-file key is ignored. `DressupConfigWire` fields are required except 6 (`config.rs:~935-962`), so an older file missing e.g. `feed_ramp_rate` fails to load.
15. `OperationError::Other(String)`/`SimulationError::ObstacleRefused(String)` flatten typed refusals to strings (`simulate.rs:~855`); `session/compute/generation.rs:772` builds `Other(format!..)` by hand. in flux (generation.rs).
16. `operation_spans` re-export (`mod.rs:59`) has zero outside callers; `spans_from_depth_runs`, `spans_from_drill_holes`, `spans_from_move_intents` are `pub` with test-only outside use; `feed_optimization_unavailable_reason` is re-exported twice in viz (`state/toolpath.rs:9`, `state/toolpath/catalog.rs:6`).
17. `toolpath_stats.rs:1-8` and `execute/findings.rs`: `GenerationFindings` (21 fields) and `ToolpathStats` (27) mirror each other; pairing is exhaustive-destructure (good) but `RecordFindings` impls for 4 findings live in `findings.rs:441-510` as inherent impls on types owned by `toolpath_stats.rs` (split ownership).
18. `ExecutionContext` has 22 `pub` fields (`execute.rs:318-424`) incl. `findings: &RefCell` that any adapter may mutate directly; the "write only via `record`" rule is convention.
19. `ComputeStatus::label()` returns a free string used as an MCP wire value (`config.rs:~108`); `AwaitingPriorStock` is Serialize-only with a doc saying "Nothing reads the shape back".
20. `DressupConfig::is_dressup_enabled(key:&str)` answers `false` for any unknown key (`config.rs:1220-1228`) - fails open; `owner_of_value_field` likewise `None`.
21. `OpPolicy::MILLING` as the "ordinary" constant (`schema.rs:492`): a new op that copies `MILLING` gets `empty_is_feature_selective:false`, `raster_stepover_is_lateral:false` by default without an explicit decision (the CMP-07 aim was explicit decisions).
22. `tool_config.rs:940-1280` name-vs-geometry heuristic parser (inch fractions, unit scans, 10% tolerance) inside the model file; tolerances `NAME_MATCH_REL_TOL=0.10`, `NAME_TIP_REL_TOL=0.02` are magic and "never a number from memory" would apply to their provenance.
23. `ALL_2D`/`ALL_3D` (`catalog.rs:470-495`) are hand lists parallel to the macro's category column; `ALL_3D` has one outside user (`crates/rs_cam_viz/src/ui/toolpath_panel.rs:1167`).
24. `execute.rs:638-678` four `#[cfg(test)] #[allow(...)] mod x;` stanzas and `execute/tests.rs` 1688 lines: test files named by ticket (`pinned_bottom_z_reaches_motion_g_bottompin.rs`) in the production folder.
25. `validate.rs:353-365` re-derives the stock bottom and tool lookup from a session instead of the heights resolver (`compute_stock_bottom_z`); possible drift from `HeightsConfig::resolve`. in flux.
