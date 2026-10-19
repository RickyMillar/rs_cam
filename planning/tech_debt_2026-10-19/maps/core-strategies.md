# Map: core-strategies

Scope: `crates/rs_cam_core/src/{dressup,adaptive,adaptive3d,ops,surface}/`. Evidence baseline: HEAD 99b6fb41. All paths below are under `crates/rs_cam_core/src/` unless they start with `crates/` or `planning/`. The map is read-only evidence. "Unverified" means the author did not run the code.

Measurement caveats (read once):
- Most files here hold an inline `#[cfg(test)] mod tests`. Line counts below are whole-file counts. Production size is smaller where noted.
- Counts of `unwrap`, `panic` and `allow(` made with rg include test modules. This map cites the instrument `planning/tech_debt_2026-10-19/instruments/allows_without_safety.md` for production allow counts (120 rows in scope).
- Caller counts are from rg on `*.rs`, excluding `tests/` and `benches/` unless a column says otherwise. The graph tools were not used.
- In flux (uncommitted, other sessions edit them): `crates/rs_cam_core/tests/adaptive3d_commanded_ladder.rs`, `adaptive3d_entry_stock_aware.rs`, `adaptive3d_interior_cell_parity_f029.rs`, `adaptive3d_lift_bridge_b1.rs`, `adaptive3d_planner_stock_xy_f027.rs`, `adaptive3d_subtool_channel_gouge.rs`, `adaptive_feed_modulation_pipeline_f036b.rs` (in_flux_paths.txt:29-35). No source file of this scope is in flux. The sentries `adaptive3d_entry_stock_aware` and `adaptive3d_subtool_channel_gouge` are named in the folder CLAUDE.md files, so treat their status as in flux.

## 1. Purpose and file map

The area holds the geometry engines that turn a polygon, a mesh or a hole list into a `Toolpath`. It also holds the post-generation transforms (dressups) and the cutter-walk surface fields that finishing uses. `adaptive/` and `adaptive3d/` are the two clearing planners. `ops/` is one module per 2.5D, engraving or drilling operation, plus the shared Z ladder. `surface/` is the drop-cutter and push-cutter walks and the derived rest, slope and reach fields. `dressup/` holds the passes that `compute/execute/dressup_apply.rs` runs after generation. The area has no registry and no session access. The registry, the config types and the dispatch live in `compute/`.

### dressup/ (12,335 lines incl. 2,163 test lines)

| file | lines | responsibility |
|---|---|---|
| dressup/mod.rs | 2636 | Facade plus seven passes in one file: entry config types (`EntryStyle` :87, `EntrySurfaceProbe` :112, `EntrySafety` :169), `apply_entry` :368, `optimize_entry_descents*` :698-790, tabs :1099-1148, `apply_lead_in_out` :1449, `apply_dogbones` :1809, `apply_link_moves` :1991, `reference_engagement_of_cutting_moves` :2197, `filter_air_cuts` :2357 |
| dressup/tests.rs | 2163 | Unit tests moved out of mod.rs |
| dressup/feed_modulation.rs | 1758 | F-039 chipload modulator: `ModulationStrategy`, `ChiploadBand`, `adaptive_feed_modulate*` :809-855. Tests from :1006 |
| dressup/tsp.rs | 1650 | Rapid reorder (`optimize_rapid_order` :206). Tests from :799 |
| dressup/arcfit.rs | 1543 | Lines to arcs (`fit_arcs` :79, `fit_arcs_within` :90). Tests from :784 |
| dressup/entry_descent.rs | 896 | Ramp, helix and stock-aware plunge emitters (`emit_ramp` :482, `emit_helix` :762) |
| dressup/feedopt.rs | 588 | `optimize_feed_rates` :134 (geometry feed caps) |
| dressup/condition.rs | 420 | `merge_linear_runs` :51 (segment merge) |
| dressup/entry_audit.rs | 265 | `buried_fed_chords` :59, `fed_moves_outside_region` :189 (audits; only tests call them) |
| dressup/air_cut.rs | 197 | Private air-sampling helpers for `filter_air_cuts` |
| dressup/link.rs | 179 | `bridge_corridor_is_swept` :96 for `apply_link_moves` |

### adaptive/ (8,745 lines)

| file | lines | responsibility |
|---|---|---|
| adaptive/path.rs | 3334 | Orchestrator, capped walker, frontier hop, mop, contour-parallel cleanup, starter pockets, emission, `simplify_path` :2902. Production ends near :3160 |
| adaptive/mod.rs | 2662 | Only about 430 lines are production: params (`AdaptiveParams` :181), enums, `adaptive_toolpath*` :323-360, `inset_piece_outlines` :388. Lines 433-2662 are inline tests |
| adaptive/search.rs | 1628 | Step measures, `PassLoad` :735, `ToolCentreRegion` :28, direction search :1005, entry finders |
| adaptive/material_grid.rs | 667 | 2D material grid (`MaterialGrid` :11) |
| adaptive/spiral.rs | 414 | Contour spiral generator |

### adaptive3d/ (9,727 lines incl. 2,546 test lines)

| file | lines | responsibility |
|---|---|---|
| adaptive3d/clearing.rs | 3186 | Z-level engine: `AreaMask`, `PlannerCursor`, stamping, `plan_entry` :757, `detect_and_order_regions` :1871, `LevelSink` :2045, `planner_outlines` :2165, `clear_one_region` :2377, `coalesce_level_entries` :2858, `clear_z_level_adaptive` :2884. Tests from :3106 |
| adaptive3d/path.rs | 2179 | Z levels (`step_levels` :348), level loop, `segments_to_toolpath`, stay-down links, runtime labels. Tests from :1702 |
| adaptive3d/tests.rs | 2546 | Unit tests |
| adaptive3d/mod.rs | 653 | Param groups, enums, 8 entry-point wrappers :488-640 |
| adaptive3d/centre_clip.rs | 471 | Session-boundary clip before each stamp |
| adaptive3d/search.rs | 376 | Clear-path test (private) |
| adaptive3d/region_map.rs | 276 | By Area region map (`AreaRegionMap` :50, serialize-only) |

### ops/ (8,790 lines)

| file | lines | responsibility |
|---|---|---|
| ops/pocket.rs | 924 | Pocket ring cascade, `PocketCascadeReport`, `CascadeBound` |
| ops/waterline.rs | 814 | Constant-Z contours, ladder, `WaterlineLevel`. A Menu3d op in a folder documented as "2.5D and drilling" |
| ops/project_curve.rs | 735 | Project curve op plus 3 enum mirrors of config enums |
| ops/face.rs | 728 | Face op, `face_levels` :122 |
| ops/drill_metrics.rs | 682 | Drill samples, chip-welding risk, summaries (read by `tool_load/`) |
| ops/inlay.rs | 621 | Inlay pocket and plug |
| ops/vcarve.rs | 601 | V-carve scan lines (rayon behind `parallel`) |
| ops/zigzag.rs | 582 | Zigzag lines; adaptive also uses it |
| ops/depth.rs | 572 | The one Z ladder (`z_ladder` :60, `LadderMode` :19), `DepthStepping` :116 |
| ops/drill.rs | 524 | Drill cycles, `drill_toolpath*`, `fed_descents` |
| ops/chamfer.rs | 476 | Chamfer rings |
| ops/trace_path.rs | 462 | Follow-path op |
| ops/profile.rs | 416 | Profile op |
| ops/adaptive_shared.rs | 406 | Engagement math and corner blending shared by `adaptive/` and `adaptive3d/` |
| ops/drill_op.rs | 184 | `DrillOp` data model and `OpData` (a session cache payload) |
| ops/mod.rs | 23 | Facade, 15 `pub mod` lines |

### surface/ (9,218 lines)

| file | lines | responsibility |
|---|---|---|
| surface/rest_field.rs | 2974 | Rest-depth field and pencil valley detector (`detect_rest_valleys` :1032), thinning, NMS. Production ends near :2015 |
| surface/slope.rs | 1252 | `GridZ` :30, `SurfaceHeightmap` :92, `SlopeMap` :458, `classify_steep_shallow` :781 |
| surface/dropcutter.rs | 1200 | `batch_drop_cutter` :97, `DropCutterGrid` :68, `point_drop_cutter` |
| surface/pushcutter.rs | 1084 | Push-cutter fiber walk |
| surface/reach.rs | 1011 | Canonical valley-reach policy |
| surface/merge_tree.rs | 544 | Pocket join tree. No product reader |
| surface/chord_refine.rs | 541 | Splits a feed that leaves the drop-cutter surface (`pub(crate)`) |
| surface/flow_accum.rs | 402 | D8 flow routing. No product reader |
| surface/stock_reference.rs | 155 | `StockReference::resolve` :54 |
| surface/mod.rs | 15 | Facade |

## 2. Public entry points and outside callers

Production counts exclude `tests/` and `benches/`. "Tests" is the number of hits in `tests/` dirs.

| entry point | defined | outside production callers | tests |
|---|---|---|---|
| `adaptive::adaptive_toolpath_structured_annotated_traced_with_cancel` | adaptive/mod.rs:348 | 1: compute/execute/clearing_2d.rs:720 | 3 |
| `adaptive::adaptive_toolpath` | adaptive/mod.rs:323 | none outside the area | 20 |
| `adaptive3d::adaptive_3d_toolpath_output_with_cancel` | adaptive3d/mod.rs:571 | 1: compute/execute/rough_3d.rs:196 | 11 |
| other 7 `adaptive_3d_toolpath*` wrappers | adaptive3d/mod.rs:488-640 | `_annotated` has 0 production callers; the others have 1-3 each, mostly in-area | 2-41 each |
| `compute::execute::apply_dressups` (drives dressups) | compute/execute/dressup_apply.rs | session/compute.rs:855 | 82 |
| `dressup::apply_entry` | dressup/mod.rs:368 | dressup_apply.rs (entry stage) | many |
| `dressup::optimize_entry_descents_annotated` | dressup/mod.rs:738 | session/compute.rs:1038 (one call, outside the pipeline) | 25 |
| `dressup::apply_tabs` | dressup/mod.rs:1130 | compute/execute/clearing_2d.rs:510 (one call, outside the pipeline) | 8 |
| `dressup::apply_lead_in_out` / `apply_dogbones` / `apply_link_moves` / `filter_air_cuts` | dressup/mod.rs:1449 / 1809 / 1991 / 2357 | each: 1 call in dressup_apply.rs (:731, :706, :767, :878) | 20 / 10 / 10 / 13 |
| `dressup::arcfit::fit_arcs_within` | dressup/arcfit.rs:90 | dressup_apply.rs:793 | 43 (via `fit_arcs`) |
| `dressup::tsp::optimize_rapid_order` | dressup/tsp.rs:206 | dressup_apply.rs:522, :853 | 15 |
| `dressup::condition::merge_linear_runs` | dressup/condition.rs:51 | dressup_apply.rs:832 | 10 |
| `dressup::feedopt::optimize_feed_rates` | dressup/feedopt.rs:134 | dressup_apply.rs:927 | 10 |
| `dressup::feed_modulation::adaptive_feed_modulate_with_materials` | dressup/feed_modulation.rs:855 | session/compute.rs:2080 | 27 (for `adaptive_feed_modulate`) |
| `dressup::feed_modulation::adaptive_feed_modulate` | dressup/feed_modulation.rs:809 | 0 production callers | 27 |
| `surface::dropcutter::batch_drop_cutter` | surface/dropcutter.rs:97 | 1: finish/steep_shallow.rs:393 | not counted |
| `surface::dropcutter::point_drop_cutter` | surface/dropcutter.rs | 18, e.g. maps/reach_map.rs:1402, maps/tier_flute_reach.rs:147, stock/collision.rs:354, dressup/mod.rs:31 | not counted |
| `surface::rest_field::detect_rest_valleys` | surface/rest_field.rs:1032 | 1: finish/rest_detector.rs:127 | not counted |
| `surface::slope::classify_steep_shallow` | surface/slope.rs:781 | finish/steep_shallow.rs:710 | |
| `surface::stock_reference::StockReference::resolve` | surface/stock_reference.rs:54 | compute/execute/territory.rs:223, compute/execute/drilling.rs:265 | |
| `surface::reach::{suggested_offset_stepover_mm, offset_passes_per_side}` | surface/reach.rs | finish/crease_paths.rs:135, :157, :175 (7 call sites in total) | |
| `ops::*` runners | ops/*.rs | called from `compute/execute/*.rs` adapters through `OperationType::registry_entry().generate` (compute/execute.rs:593-596) | `tests/common/kernels.rs`, `op_fixtures::generated_op` |

Folder-level fan-in (production files outside the folder that name `crate::<folder>`): ops 42 files (compute 13, stock 4, session 4, adaptive 4, gcode 3, finish 3, adaptive3d 3), surface 49 (finish 23, compute 7, maps 5, adaptive3d 4), dressup 20 (compute 6, session 3, trace 2, tool_load 2, finish 2), adaptive 8, adaptive3d 5.

External crates: `rs_cam_cli` uses `rs_cam_core::dressup` in 5 files (rough_score.rs, project.rs, main.rs, smoke.rs). `rs_cam_viz` uses `ops` in 8 files, `surface` in 4 UI files, `adaptive3d` in 3, `adaptive` in 1 (ui/properties/operations/boundary_2d.rs), `dressup` in 1 (controller/events/compute.rs). `rs_cam_mcp` has no import.

Dead or test-only public surface (instruments `dead_pub_surface.md`, `test_only_pub_api.md`): `adaptive_3d_toolpath_structured_annotated_traced_with_cancel` (adaptive3d/mod.rs:618), `OutsideRegionChord` (entry_audit.rs:142), `ReferenceEngagement` (dressup/mod.rs:2171), waterline consts and fns (waterline.rs:112-118, :140, :336), `adaptive_3d_toolpath_annotated` (test only), `flow_accum::{resolve_flats, d8_accumulation}`, `GridZ::is_covered`.

## 3. Dependencies and layering

Imports from production code (rg of `crate::<mod>`; top items):

| area folder | imports from | notes |
|---|---|---|
| dressup | toolpath (49), geo, tool, trace::toolpath_spans (11), trace::transform_provenance, tool_load (9), machine::{kinematic_utilization, kinematics}, geometry, finish::pencil, stock, polygon, dexel_stock, feeds::force, material, compute::config, surface::dropcutter | Heaviest cross-layer reach |
| adaptive | geo, toolpath, polygon, ops::adaptive_shared (10), ops::zigzag, trace::debug_trace, stock::dexel, interrupt, dressup::contained_helix_radius, compute::spans | |
| adaptive3d | adaptive (31), mesh (19), polygon, toolpath, tool, surface::{slope, dropcutter}, compute::operation_configs (6), dressup::{emit_ramp, emit_helix}, trace::produced, compute::spans | |
| ops | toolpath, geo, polygon, compute::operation_configs (9), interrupt, ops siblings, tool_load::boundary, stock::simulation_cut, maps::reach_map, surface::{pushcutter, dropcutter} | |
| surface | surface::reach (internal), geometry::{grid2, region_mask}, tool, mesh, measurement, finish::* | `finish::*` hits are doc links (surface/rest_field.rs:133, :318, :342, :1611, :2011, :2037) and a test module (rest_field.rs:2868-2906, test module starts :2158). Production code is clean. surface/mod.rs:5 states "depends downward only" and this holds |

Layering inversions and cycles:

| # | finding | evidence |
|---|---|---|
| L1 | Cycle dressup <-> tool_load | dressup/feed_modulation.rs:66 and :69 import and re-export `tool_load::{PowerModelInputs, BindingConstraint}`. tool_load/mod.rs:192 and :247 import `dressup::feed_modulation::ChiploadBand` |
| L2 | The toolpath IR depends on a surface type | toolpath.rs:8 imports `surface::dropcutter::DropCutterGrid`. The root CLAUDE.md names the IR as the planning/post boundary |
| L3 | Trace layer depends on strategy types | trace/produced.rs:18 imports `surface::rest_field::RestGrid`; :51 imports `adaptive3d::AreaRegionMap`. maps/rest_heatmap_mesh.rs:451 imports `adaptive3d::AreaRegionMap` |
| L4 | Data model in `ops/` read by lower layers | stock/dexel_mesh.rs:453, dexel_stock/mod.rs:752 use `ops::drill_op::DrillOp`. stock/simulation_cut.rs:2, stock/simulation_cut/analysis.rs:10, trace/narrate.rs:1965 use `ops::drill_metrics`. stock/sim_triage.rs:741 uses `ops::depth::realised_step_down` |
| L5 | Engine imports config enums from `compute/` | adaptive3d/mod.rs:89, :101 and ops/project_curve.rs:67-88 write `From<compute::operation_configs::X>` impls. dressup/mod.rs:447 reads `compute::config::DRESSUP_RAMP_ANGLE_DEG`. `compute/operation_configs.rs:661-675` imports the `adaptive::{CleanupStrategy, EngagementMeasure, PathStrategy2d}` enums back. The dependency runs both ways |
| L6 | Sibling engines depend on each other and on `ops/` | adaptive3d imports adaptive (31 uses). adaptive imports `ops::adaptive_shared` and `dressup::contained_helix_radius`. adaptive3d calls dressup `emit_ramp` / `emit_helix` directly (the "adaptive3d door", dressup/mod.rs:175-185) |
| L7 | dressup -> finish::pencil in production | dressup/entry_descent.rs:520-541, dressup/mod.rs:941. This is a one-way dependency, not a cycle. It looks sanctioned by the dressup CLAUDE.md |

## 4. Traits

The area defines no trait (rg `trait ` over the five folders returns nothing). It implements these external traits:

| trait | implementors here |
|---|---|
| `crate::compute::spans::RuntimeLabel` | `AdaptiveRuntimeAnnotation` (adaptive/path.rs:3157), `Adaptive3dRuntimeAnnotation` (adaptive3d/path.rs:1692) |
| `std::cmp::{Ord, PartialOrd, Eq}` | `HeapItem` (surface/flow_accum.rs:104-112) |
| `Default` | `RestFieldParams` (surface/rest_field.rs:170), `ProjectCurveParams` (ops/project_curve.rs:125) |
| `From<config enum>` | `RegionOrdering`, `ClearingStrategy3d` (adaptive3d/mod.rs:89, :101); `ProjectDirection`, `ProjectSide`, `ProjectProjection` (ops/project_curve.rs:67, :77, :88) |
| derives `rs_cam_derive::ParamEnum` + serde | `CleanupStrategy`, `EngagementMeasure`, `PathStrategy2d` (adaptive/mod.rs:79, :113, :142); `FaceDirection` (face.rs:20), `ProfileSide` (profile.rs:19), `TraceCompensation` (trace_path.rs:19) |

Traits used as parameters: `crate::tool::MillingCutter` (`&dyn MillingCutter` in `EntrySurfaceProbe.cutter`, dressup/mod.rs:112; generic `C: MillingCutter + ?Sized` in surface/dropcutter.rs:97) and `crate::interrupt::CancelCheck` (`&dyn CancelCheck`). The area does not use `OpParams`. The area's `*Params` structs are plain engine structs. The `OpParams` derive lives on the `*Config` types in `compute/operation_configs.rs`.

## 5. Recurring patterns

Grades: TYPES | LINT | SENTRY | DOC | CONVENTION | NONE.

| concern | pattern used here | example | grade | deviations inside the area |
|---|---|---|---|---|
| Errors and refusals | Engines almost never return `Result`. Cancel is `Result<_, Cancelled>` (a unit error) in 8+ engines. A refusal is a silent empty output, an `Option`, a bare count (`(Vec<..>, usize)` offset-failure count, `*_reported` suffix) or a `tracing::warn!`. `ModulationError` (a 2-variant `Copy` enum) is the one error enum. Production panics are denied by workspace lints; each test or algorithm module opts out with `#[allow]` | ops/zigzag.rs:49-56 (`(lines, usize)`); dressup/feed_modulation.rs:346; interrupt.rs:70-73 (`run_uncancellable` has the one production `expect`) | LINT for panics; NONE for the refusal shape | `Vec<(usize, String)>` annotation channel (adaptive3d/mod.rs:541, :553, :641). `surface/dropcutter.rs` abstains by returning `contacted: false` (CLAUDE.md invariant "off the mesh must abstain", enforced by test `drop_cutter_off_mesh`). `StockReference::Absent` (stock_reference.rs:41) is a typed refusal with a reason |
| State mutation | Absent. No `ProjectSession`, no `Command`. Functions take inputs and return a `Toolpath` or `Transformed`. `ops/drill_op.rs` `OpData` is a payload type that session code stores. `adaptive_feed_modulate` mutates `&mut Toolpath` in place | dressup/mod.rs:1130 (`apply_tabs(Toolpath,..) -> Toolpath`); dressup/feed_modulation.rs:809 | TYPES (the area imports no session type; the dependency does not exist) | `adaptive_feed_modulate*` takes `&mut Toolpath` (feed_modulation.rs:809), unlike the by-value dressups. `apply_tabs`, `apply_lead_in_out` etc. differ in return type: `Toolpath` vs `Transformed` (see Provenance row in section 6) |
| Parameters and config | One plain `pub struct XParams` per engine, all `pub f64` / `bool` fields, built by the `compute/execute` adapter from an `XConfig`. No validation in the engine. `Default` exists on 2 of about 20 structs. `Adaptive3dParams` is grouped (`geometry`, `depth`, `linking`; CUT-04) | adaptive3d/mod.rs:128-330; ops/pocket.rs:19-37 | CONVENTION (grouping rule is DOC at adaptive3d/CLAUDE.md:15); the `..*params` restamp advice is a doc comment (pocket.rs:60-62) | Some structs are `Copy` (`PocketParams`), others are not (`VCarveParams`, `Adaptive3dParams`). `AdaptiveParams` is flat (adaptive/mod.rs:181). Struct-update `..*params` is the stated rule for restamp but needs `Copy` |
| Units and frames | Bare `f64`. mm everywhere; angles carry a suffix: `_deg` (`max_angle_deg`, `angle_deg`) or `_rad` (`cone_half_angle_rad`). Frames are named in doc comments only ("emission frame", "tool-centre space", "rotated frame"). `top_z`, `stock_top_z`, `z_floor` are caller-supplied raw `f64` | ops/vcarve.rs:26 (`half_angle` radians, no suffix); adaptive3d/mod.rs:178 (`world_stock_xy_bbox: Option<(f64,f64,f64,f64)>`); surface/dropcutter.rs:51 (rotated frame doc) | DOC / NONE | `VCarveParams.half_angle` is radians while every other angle field is degrees (vcarve.rs:26). `ZigzagParams.angle` is degrees without suffix (zigzag.rs:30). `FaceDirection` / `FACE_ANGLE` (face.rs:104). `[f64; 4]` rect in `AreaRegionMap::run_rect` (region_map.rs:219). 2D tuples `(f64, f64)` in adaptive3d/clearing.rs:1739-1760 |
| Identifiers | Almost none cross the area. `ToolpathId` newtype only in ops/drill_metrics.rs:15,28. Move and span indices are `usize`; region order is `u16` (region_map.rs:186) | ops/drill_metrics.rs:28 | TYPES where used, NONE elsewhere | `line_index: usize` in runtime events (adaptive/mod.rs:229+); `Vec<(usize, String)>` carries a move index plus a label |
| Background work, progress, cancellation | Every long engine has a `*_with_cancel(&dyn CancelCheck)` core returning `Result<_, Cancelled>` and a thin non-cancel wrapper through `run_uncancellable`. `check_cancel(cancel)?` between cells, rings or levels. No progress channel in the area; `grep progress` hits only comments and `ops/drill*`. `rayon` behind feature `parallel` in vcarve, inlay, dropcutter, rest_field | adaptive/mod.rs:323-335; ops/vcarve.rs:136, :153 | CONVENTION (the `run_uncancellable` shim hides the cancel branch; there is no type that forces a cancel poll in a new loop) | Some engines have only the `_with_cancel` form (`adaptive_3d_toolpath_traced_with_cancel`, `#[cfg(test)]` at adaptive3d/mod.rs:514). Entry-point wrapper counts differ per engine (adaptive 4, adaptive3d 8, ops 1-3). `surface/slope.rs` has 8 cancel references and `run_uncancellable` at :128 |
| Caching, freshness, invalidation | Absent. The engines rebuild every call. `OpData` (ops/drill_op.rs:158) states the dual-representation invariant in a doc comment; the cache and its invalidation live in `session/`. Only `invalidate_bbox` (adaptive3d/clearing.rs:2199) and `thread_local` test counters (arcfit.rs:38) exist | ops/drill_op.rs:150-157 | DOC | `RestGrid` is rebuilt "on every generate" (rest_field.rs:342-343 doc). The adaptive planners hold an in-flight stock replay (`PlannerCursor`), not a cache |
| Diagnostics, warnings, messages | `tracing::{debug,info,warn}` in adaptive3d (path.rs, clearing.rs), dressup/tsp.rs, stock_reference.rs. Per-engine `*RuntimeEvent` enums with a `label() -> String` and a `RuntimeLabel` impl. `ToolpathDebugContext::add_annotation(move_index, label)` for the debug trace. Dressups report through `DressupStage` constants. A finding/report struct exists only in ops (`PocketCascadeReport`, pocket.rs:87) | adaptive/mod.rs:229-310; dressup/mod.rs:2271 | CONVENTION | `label()` returns a `String` in both runtime-event enums (adaptive/mod.rs:266, adaptive3d/mod.rs:416); label text is the wire. Three dressup stages share `debug_key: "entry_style"` (dressup_apply.rs:173-190) |
| Serialization and wire types | Serde is rare. `ParamEnum` enums carry `Serialize/Deserialize` and are embedded in configs; drill types in ops/drill*.rs; `AreaRegion*` is `Serialize` only. No MCP type is defined here | adaptive/mod.rs:76-79; ops/drill.rs:38-39; adaptive3d/region_map.rs:18 | TYPES (derive), SENTRY for the wire spelling in `compute/catalog` | `ClearingStrategy3d`, `RegionOrdering`, `EntryStyle3d`, `dressup::EntryStyle` have no serde; each mirrors a config enum that does (L5) |
| Numeric tolerances and epsilons | Named `const` per module in the form `*_EPS_MM`, `*_TOL`, `*_EPSILON`, plus many inline `1e-6`/`1e-9` literals. 22 named consts found in the five folders; the other literals are unnamed | dressup/mod.rs:48, :53, :62 (three `0.01` consts); ops/depth.rs:52; ops/waterline.rs:83 | CONVENTION | `XY_EPS_MM` is defined twice with the same value in dressup/mod.rs:801 and :2621. Ladder epsilon exists as `Z_LADDER_DEFAULT_EPSILON` 0.01 (depth.rs:52) and `WATERLINE_LADDER_EPSILON` 1e-10 (waterline.rs:83); the waterline doc points to a wrong path (waterline.rs:78). `1e-` literals: pushcutter.rs 43, feed_modulation.rs 42, project_curve.rs 32 (whole-file counts) |
| Geometry primitives | `crate::geo::{P2, P3}` (nalgebra-style points), `crate::polygon::Polygon2` (exterior + holes, cavalier offset behind `offset_polygon*`), `crate::mesh::{TriangleMesh, SpatialIndex}`, `crate::dexel_stock::TriDexelStock`, `crate::maps::grid::GridSpec`. Grids keep their own `rows/cols/cell_size` in `SurfaceHeightmap`, `SlopeMap`, `MaterialGrid` (surface/CLAUDE.md says "on purpose") | adaptive/mod.rs:34; surface/rest_field.rs:250 (`grid: GridSpec`) | CONVENTION | Local re-implementations: `polygon_signed_area` (adaptive3d/clearing.rs:1787) vs `Polygon2::signed_area` (polygon.rs:221); `nearest_neighbor_order` twice (dressup/tsp.rs:482, adaptive3d/clearing.rs:1760); point-segment distance in dressup/link.rs:49 and adaptive/search.rs:844 vs geo.rs:381/393; `simplify_path` (adaptive/path.rs:2902) vs `rdp` (polygon.rs:1048) |
| Test style and sentries | (a) inline `#[cfg(test)] mod tests` with `#![allow(unwrap/expect/panic/indexing)]` on the module; (b) `tests.rs` side files (dressup, adaptive3d); (c) integration tests in `crates/rs_cam_core/tests/` named `<what_holds>_g_<gate>` (G-ids), listed per folder in CLAUDE.md. Test-only items in production files behind `#[cfg(test)]`: `adaptive_segments` (adaptive/path.rs:639), `detect_material_regions` (adaptive3d/clearing.rs:89), `adaptive_3d_toolpath_traced_with_cancel` (adaptive3d/mod.rs:514), `total_rapid_distance` (tsp.rs:146), `RUN_SCAN_READS` (arcfit.rs:38). `#[cfg(feature = "test-support")]` for `GridZ::is_covered` (slope.rs:77). All 48 sentry files named by the five CLAUDE.md files exist | dressup/tests.rs:1-12; surface/CLAUDE.md:38 | SENTRY | Inline test counts: dressup 143, ops 141, surface 93, adaptive 62, adaptive3d 59. adaptive/mod.rs holds 2,230 test lines in the facade. `adaptive/search.rs:195` and `adaptive3d/search.rs:223` have `#[cfg(test)]` followed by an `#[allow(` block that gates a test module |
| Naming and module layout | `<engine>_toolpath` + `_with_cancel` + `_traced` + `_annotated` + `_structured` suffix stack; `*_reported` for variants that return a failure count; `Params` / `Config` split (engine vs UI). Folder CLAUDE.md per folder, 40 lines | adaptive3d/mod.rs:488-640 (8 variants); adaptive/mod.rs:323-348 (4) | CONVENTION | `pub` where `pub(crate)` would do: `adaptive_toolpath_structured_annotated_traced_with_cancel` is `pub` (adaptive/mod.rs:348) but its sibling wrappers are `pub(crate)`. Only `debug_adaptive_3d_segments_for_f029_probe` (adaptive3d/path.rs:573, re-exported mod.rs:38) is a probe made public for one test |
| Registration of a new feature | Outside the area. An op is one row in `for_each_op!` (compute/catalog.rs:305-333), one `REG_*` static (compute/catalog/registry.rs:40+), one `XConfig` with `#[derive(OpParams)]` (compute/operation_configs.rs), one `generate_*` adapter (compute/execute/), and one engine module in `ops/`. `ops/mod.rs` is a hand-written `pub mod` list | compute/catalog.rs:510 (`registry_entry` exhaustive match) | TYPES for the op list; CONVENTION for engine placement | `ops/mod.rs` has no registry. `waterline.rs` lives in `ops/` while the other Menu3d ops live in `finish/` |
| Dressup provenance (added row: the main compiler-held rule here) | A dressup that moves indices returns `Transformed<Unreconciled>`; `reconcile(&mut ReconcileSet)` is the only way to get the toolpath out | trace/transform_provenance.rs:329-389; dressup/mod.rs:75 (`without_provenance`) | TYPES | `apply_tabs` (dressup/mod.rs:1130), `apply_dogbones` and `adaptive_feed_modulate` sit outside `Transformed` or run outside `DRESSUP_PIPELINE` |

## 6. Extension points

| item to add | files touched | what the compiler forces |
|---|---|---|
| New operation | `compute/catalog.rs` `for_each_op!` row (:305-333); `registry_entry` match arm (:510); `REG_*` static in `compute/catalog/registry.rs` (carries `generate` fn pointer, `params: XConfig::PARAMS`, `Capabilities` with 25+ fields); `XConfig` with `#[derive(OpParams)]` in `compute/operation_configs.rs`; `generate_x` adapter in `compute/execute/`; engine module in `ops/x.rs` plus `pub mod` in `ops/mod.rs`; sentry in `tests/`; `FEATURE_CATALOG.md`; GUI panel in `rs_cam_viz/src/ui/properties/operations/` | Forced: `registry_entry` is exhaustive, so a variant without a row does not compile. Not forced: `ALL_2D` / `ALL_3D` are hand-written (sync-tested by `operation_partitions_cover_all_variants_once`, SENTRY); the `ops/` module itself; `honors_pinned_bottom_z`, the 25 `Capabilities` values (any value compiles) |
| New dressup stage | `DressupStage` const and `DRESSUP_PIPELINE` entry (compute/execute/dressup_apply.rs:165-262); a call through `apply_dressup_traced`; the transform in `dressup/`; config field in `compute/config.rs` `DressupConfig` | Forced: the transform must return `Transformed`, and reconcile is the only exit (TYPES). Not forced: pipeline order (DOC + `dressup_span_invariants` SENTRY); membership of `DRESSUP_PIPELINE`; three dressups run outside the list (`apply_tabs`, `optimize_entry_descents_annotated`, `adaptive_feed_modulate_with_materials`); "Add no fourth" is DOC only |
| New entry style | `dressup::EntryStyle` (dressup/mod.rs:87); `config::DressupEntryStyle` + `to_core` (compute/config.rs:664-694); `adaptive3d::EntryStyle3d` (adaptive3d/mod.rs:116) and the map at compute/execute/rough_3d.rs:68; `compute/catalog/schema.rs:525` `EntryStylePolicy`; emitters in `dressup/entry_descent.rs`; `DressupStage::*_ENTRY` | Forced: matches on these enums are exhaustive. Not forced: the four enums stay in step by hand |
| New `ClearingStrategy3d` variant | adaptive3d/mod.rs:71; the `From<config::ClearingStrategy>` impl (:101); dispatch at adaptive3d/path.rs:438-464; config enum in `compute/operation_configs.rs:124` | Forced: path.rs:438 full match and the `From` impl. Not forced: adaptive3d/clearing.rs:2968 uses `matches!(.., ContourSpiral)`, so a new variant silently takes the `Agent` branch |
| New `Adaptive3dParams` dial | adaptive3d/mod.rs group struct (`geometry`/`depth`/`linking`); adapter at compute/execute/rough_3d.rs; config; catalog schema | Forced: construction sites use struct literals, so a missing field does not compile. Not forced: which group the field joins (DOC adaptive3d/CLAUDE.md:15) |
| New `AdaptiveParams` field | adaptive/mod.rs:181; adapter compute/execute/clearing_2d.rs; adaptive/path.rs:659 (literal) and adaptive3d/clearing.rs:2960-2972 (literal) | Forced: struct literals. Behavioural meaning is not forced |
| New op parameter | `XConfig` field (`#[param(...)]`) in `compute/operation_configs.rs`; `XParams` field in `ops/x.rs`; adapter mapping; `Default`; project-file serde; MCP schema regenerates from `OpParams` | Forced: struct-literal construction of `XParams`. Not forced: the adapter copying the config value (a field can default silently with `..*params`) |
| New dressup toggle on the chain | `DressupConfig` (compute/config.rs), `DressupContext` (dressup_apply.rs:468), `Capabilities` / `OperationTransformCapabilities` (e.g. `planner_applies_segment_merge`, `stock_aware_plunge`, `helix_floor_lap`) | Forced: struct literals for `DressupContext`. Not forced: whether an op row sets the capability correctly |
| New Z ladder | `ops/depth.rs` `LadderMode` | Forced: `LadderMode` match is exhaustive. CLAUDE.md "Build no third" is DOC |
| New surface analyser | `surface/x.rs` + `pub mod` in surface/mod.rs; reads only lower layers (surface/mod.rs:5 DOC) | Not forced. Nothing stops `surface/` importing `finish/` or `compute/` in code (the check is a doc comment) |
| New rest-reference consumer | Must call `StockReference::resolve` (surface/stock_reference.rs:54) | Partly forced: the enum has one constructor path (SENTRY `stock_reference` lib tests) |
| New drill gate | `tool_load/drill_gates.rs` reads `ops::drill_metrics` summaries (DOC ops/CLAUDE.md:24) | Not forced |

## 7. CLAUDE.md drift

| file:line | statement | reality |
|---|---|---|
| ops/CLAUDE.md:19 | "Query `OperationType::honors_pinned_bottom_z()`" | No such method. It is a field: `op_type.capabilities().honors_pinned_bottom_z` (compute/catalog/capabilities.rs:61; used in compute/execute/pinned_bottom_z_reaches_motion_g_bottompin.rs:210). Set `true` for Adaptive3d (registry.rs:669), Waterline (:715), UnifiedFinish (:879), which matches the stated list |
| ops/CLAUDE.md:3-4 and ops/mod.rs:1 | "the 2.5D and drilling operations" | `waterline.rs` is a 3D op (Menu3d, compute/catalog.rs:319) and `ops/depth.rs` + `adaptive_shared.rs` are shared helpers. The CLAUDE.md does list waterline, so the title is the mismatch |
| ops/CLAUDE.md:4 | "reached through `compute::execute_operation_annotated`" | The fn is `compute::execute::execute_operation_annotated`, `pub(crate)` (compute/execute.rs:593) |
| dressup/CLAUDE.md:6-7 | `adaptive_feed_modulate` runs in `session/compute.rs` | Production calls `adaptive_feed_modulate_with_materials` (session/compute.rs:2080). `adaptive_feed_modulate` (feed_modulation.rs:809) has no production caller (27 test hits) |
| dressup/CLAUDE.md:22-24 | "Every linking arm calls the shared `relink_fragments` kernel in `finish/surface_link.rs`" | No file in `dressup/` references `relink_fragments`. The kernel is defined at finish/surface_link.rs:835. The rule belongs to `finish/CLAUDE.md`, or `dressup/link.rs` and `apply_link_moves` are not a "linking arm". The scope is unclear |
| dressup/CLAUDE.md "Files" | Lists files but not `tests.rs`; says `mod.rs` is "the configuration and the facade" | mod.rs holds 7 passes (about 2,600 lines) |
| adaptive/CLAUDE.md:31-32 | names `print_exact_width_along_move` as a tool | It is a `fn` in a test file (tests/a_clearing_cut_holds_the_pass_load_g_adaptpassload.rs:857), not a crate helper. Reads like an API |
| adaptive/CLAUDE.md "Files" | `mod.rs` "entry `adaptive_toolpath`, parameters, piece split" | Correct. But mod.rs is 85 % tests (433-2662) |
| surface/CLAUDE.md | "`merge_tree.rs` ... Research only; no product path reads it" | True (rg finds only tests). `flow_accum.rs` has the same status (only tests read it) but the CLAUDE.md says "Keep the module" without saying it is also unread by product code |
| CLAUDE.md (root) "Every production allow carries `// SAFETY:`" | | 120 production allows in scope have no `SAFETY` within 3 lines (allows_without_safety.md). Many carry a text like `// bounded indexing in algorithmic code`, which is not the form. Worst files: adaptive/path.rs 20, surface/slope.rs 18, adaptive3d/clearing.rs 12, adaptive/material_grid.rs 11, dressup/mod.rs 8 |
| Code docs citing deleted paths | planning/CLAUDE.md: the 2026-09-17 purge deleted these files on purpose; docs were not rewritten and `git show planning-pre-purge-2026-09-17:<path>` retrieves them | 17 distinct `planning/...` paths named in code docs of this scope no longer exist, e.g. `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md` (6 uses, adaptive3d/mod.rs doc), `planning/review_2026-07-29/MEASUREMENT_DOMAINS.md`, `planning/v3_workplan.md`, `planning/adaptive_review_2026-04.md`, `planning/unified_v3_design.md`, `planning/AGENTSEARCH_INVESTIGATION_LOG.md` |
| ops/waterline.rs:78 | intra-doc link `crate::finish::finish_setup::Z_LADDER_DEFAULT_EPSILON` | The const is `crate::ops::depth::Z_LADDER_DEFAULT_EPSILON` (depth.rs:52) |
| Sentry names | All 48 `--test` names in the five CLAUDE.md files | All exist as files in `crates/rs_cam_core/tests/` (checked). No missing sentry |

## 8. Navigability

| item | note |
|---|---|
| Files over 1,500 lines (production part) | adaptive/path.rs 3334 (about 3,160 production: orchestrator, starter pockets, mop, contour-parallel, emission and simplify in one file); adaptive3d/clearing.rs 3186 (about 3,100 production: stamping, entries, region detection, level sink, clear loop); dressup/mod.rs 2636 (all production; seven passes); surface/rest_field.rs 2974 (about 2,015 production plus 960 tests; detector, thinning, NMS, skeleton trace); adaptive3d/path.rs 2179 (1,700 production); dressup/feed_modulation.rs 1758 (1,000 production); dressup/tsp.rs 1650 (800 production); adaptive/search.rs 1628; dressup/arcfit.rs 1543 |
| Test bulk in facade files | adaptive/mod.rs (2230 test lines in a 2662-line file); adaptive3d/path.rs, clearing.rs have large inline test modules (`#[cfg(test)]` at :1702 and :3106). dressup and adaptive3d moved tests to `tests.rs`; adaptive did not |
| Mixed responsibilities | `dressup/mod.rs` (config types for entry, tabs, leads, dogbones, link, air filter, engagement helper). `surface/slope.rs` holds `GridZ`, the heightmap, the slope map and a steep/shallow classifier. `surface/rest_field.rs` mixes the field build, region pathology classification and skeleton tracing. `ops/waterline.rs` mixes the ladder, level contours and an air-bridge helper (`bridge_air_stretches`, :367) |
| Misleading names | `ops/trace_path.rs` vs `trace/` (documented in ops/mod.rs:5). `adaptive3d::RegionOrdering` vs `compute::operation_configs::RegionOrdering`. Four `EntryStyle*` enums. `Adaptive3dGeometry.tool_radius` vs `envelope_radius` (documented at :131-138). `ops/adaptive_shared.rs` is imported by `adaptive/` and `adaptive3d/` and is not an op. `surface/` holds `StockReference` (a stock guard, not a surface fact) |
| Items in the wrong folder | `OpData` (session cache payload, ops/drill_op.rs:158). `DrillOp` and `drill_metrics` are a data model read by `stock/`, `dexel_stock/`, `tool_load/`, `trace/` (L4). `DropCutterGrid` used by `toolpath.rs` (L2). `AreaRegionMap` used by `trace/` and `maps/` (L3). `waterline.rs` belongs with the other Menu3d ops in `finish/` or a 3D folder. `dressup::feed_modulation` carries the `tool_load` model types (L1) |
| Dual entry-point stacks | 8 wrappers in adaptive3d/mod.rs:488-640, 4 in adaptive/mod.rs:323-360. Callers use 4 of the 8 in production |

## 9. Candidate debt leads (UNVERIFIED)

| # | location | lead |
|---|---|---|
| 1 | adaptive3d/mod.rs:488-640 | Eight `adaptive_3d_toolpath*` wrappers; `_annotated` has no production caller and `_structured_annotated_traced_with_cancel` (:618) has none outside the area (instrument: dead). Collapse to a context struct |
| 2 | adaptive/mod.rs:348 | `pub` entry with a six-part name; the sibling wrappers are `pub(crate)`. Visibility inconsistent |
| 3 | dressup/feed_modulation.rs:809 | `adaptive_feed_modulate` has no production caller; 27 test calls keep it alive |
| 4 | dressup/mod.rs:2171, :2197; entry_audit.rs:142, :189, :59 | `ReferenceEngagement` / `OutsideRegionChord` flagged dead by the instrument; `buried_fed_chords` and `fed_moves_outside_region` are called only by tests/disconnected_finish_retract_structure_p0.rs. Audits in a production module with no production reader |
| 5 | surface/flow_accum.rs, surface/merge_tree.rs | About 950 lines with no product reader (only tests). `resolve_flats` and `d8_accumulation` are test-only pub. CLAUDE.md orders to keep `flow_accum` |
| 6 | adaptive3d/path.rs:573; mod.rs:38 | `debug_adaptive_3d_segments_for_f029_probe` is a `pub` re-export used by one test |
| 7 | adaptive3d/clearing.rs:1760, 1787, 1739; dressup/tsp.rs:482 | Duplicate `nearest_neighbor_order`, local `polygon_signed_area` (vs `Polygon2::signed_area` polygon.rs:221), local centroid |
| 8 | dressup/link.rs:49; adaptive/search.rs:844 | Local point-segment distance; the workspace has at least 8 such helpers (geo.rs:381, :393, polygon.rs:1946, finish/direction_field.rs:1701, finish/conformal_spiral.rs:2485, compute/alignment_pins/walls.rs:181, compute/execute/standing_material.rs:764) |
| 9 | adaptive/path.rs:2902 | `simplify_path` vs `polygon.rs:1048` `rdp` |
| 10 | dressup/mod.rs:801, :2621 | `XY_EPS_MM` defined twice; three different `0.01` consts at :48, :53, :62 |
| 11 | ops/depth.rs:52 vs ops/waterline.rs:83 | Two ladder epsilons (0.01 and 1e-10) with a wrong doc link (waterline.rs:78) |
| 12 | adaptive3d/clearing.rs:2968 | `matches!(.., ContourSpiral)` where a full match would force a new variant to choose |
| 13 | adaptive3d/mod.rs:46-125; ops/project_curve.rs:30-100; compute/config.rs:664 | Enum mirrors of config enums with hand-written `From`: 5 pairs. Four `EntryStyle` enums |
| 14 | adaptive3d/mod.rs:541, 553, 641 | `Vec<(usize, String)>` annotation channel next to typed `*RuntimeAnnotation` |
| 15 | adaptive/mod.rs:266; adaptive3d/mod.rs:416 | `label() -> String` for events; label text is consumed as wire |
| 16 | dressup/mod.rs:198, :210; ops/depth.rs:45; ops/pocket.rs:36; adaptive3d `z_blend`, `detect_flat_areas`; adaptive `slot_clearing` | Bool dials (`stock_top_measured`, `helix_floor_lap`, `snap_to_bottom`, `climb`) where an enum would name the arms |
| 17 | adaptive3d/mod.rs:178; region_map.rs:219 | `(f64, f64, f64, f64)` and `[f64; 4]` bbox/rect where a bbox type exists |
| 18 | ops/vcarve.rs:26 | `half_angle` in radians; other angle fields are degrees (`_deg`) |
| 19 | dressup/mod.rs:1127; ops/zigzag.rs:34 | `#[allow]` placed between doc-comment lines of one item (`apply_tabs`, `zigzag_lines`); doc and attribute order is broken |
| 20 | compute/execute/dressup_apply.rs:173-190 | Three stage constants share `debug_key: "entry_style"` (stringly key) |
| 21 | dressup/mod.rs (CLAUDE.md "Add no fourth") | Three dressups run outside `DRESSUP_PIPELINE`; DOC only |
| 22 | adaptive/mod.rs:81-90; adaptive3d/clearing.rs:2183, 2267 | `CleanupStrategy::Legacy` stays live because adaptive3d depends on it for byte parity (sentry `adaptive3d_emission_byte_parity`). The operator ruling "no legacy, breaking OK" (2026-09-16) may apply; verify the sentry before removal |
| 23 | adaptive3d/mod.rs:47, 228, 280 | Doc strings "backward compat" on defaults (legacy_residue.md) |
| 24 | toolpath.rs:8; trace/produced.rs:18, :51 | Upward dependency from the IR and trace to strategy types (L2, L3) |
| 25 | dressup/feed_modulation.rs:66, 69 | Cycle with `tool_load` (L1) |
| 26 | ops/drill_op.rs:158 | `OpData` session payload in `ops/` |
| 27 | arcfit.rs:38; adaptive/path.rs:639; clearing.rs:89 | `#[cfg(test)]` items and a `thread_local` counter inside production files; test seams in production modules |
| 28 | whole scope | 120 production `allow` without `// SAFETY:` (allows_without_safety.md); about 30 `too_many_arguments` allows in scope (rg, may include test modules) (adaptive3d/clearing.rs 10, adaptive/search.rs 3, adaptive/path.rs 3, surface/slope.rs 3) show parameter bundles missing (cf. `EntrySafety`, which fixed this for entry) |
| 29 | 17 planning paths in code docs | Pointers to purged files. Intentional per planning/CLAUDE.md (retrieve with the tag), so low value; a reader without the tag hits a dead end (section 7) |
| 30 | ops/pocket.rs:60-62 | Rule "restamp with `..*params`" is a doc comment; `PocketParams` is `Copy` but `VCarveParams`, `Adaptive3dParams` are not, so the rule does not generalise |
