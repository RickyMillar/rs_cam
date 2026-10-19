# Map: core-io-output

Scope: `crates/rs_cam_core/src/{geometry,trace,gcode,export,diagnostics,machine,tool,io}/`.
Baseline: HEAD 99b6fb41. Read-only review. All paths are relative to `crates/rs_cam_core/src/` unless they start with `crates/`.
"in flux" = the path is in `instruments/in_flux_paths.txt`. In this scope only `gcode/mod.rs` is in flux. Many `session/` files that call into this area are also in flux.
Line counts are approximate (`wc -l`, tests included). "prod" means the lines before the first `#[cfg(test)]` or `mod tests`.

## 1. Purpose and file map

Purpose. The area turns a generated `Toolpath` into output and into records about the output. It holds derived geometry (regions, grids, boundary clip), the trace records of a generation, the G-code program builder / emitter / posts, the non-G-code output (preview, ribbon, fingerprint, validator), the unified diagnostic model, the machine profile and kinematics, the cutter geometry, and the import doors with the two TOML libraries. It is not a clean layer. Several files orchestrate session state (`gcode/mod.rs`) or render pictures (`export/fingerprint.rs`). Several folders import upward (see section 3).

### geometry/ (8.1k lines)
| file | responsibility | lines |
|---|---|---|
| geometry/mod.rs | facade, `pub mod` list; `nn_order` is `pub(crate)` | 20 |
| geometry/boundary.rs | `ToolContainment`, `effective_boundary(_reported)`, user-offset outcome, the one clip walk (`clip_walk`, `ClipPermit`), toolpath clip (4 variants), model silhouette | 1746 (prod 982) |
| geometry/region_set.rs | `RegionSet` (disjoint region polygons, `Cow`) | 193 |
| geometry/region_mask.rs | mask to closed polygons, `MAX_REST_REGIONS`=64, `RegionCapReport` (3 fn variants) | 537 |
| geometry/grid2.rs | `Grid2<T>` row-major grid, bounds-checked, `GridShapeError` | 382 |
| geometry/grid_field.rs | EDT, EDT curvature, smoothing over raw `&[f64]`/`&[bool]` + `rows, cols` | 338 |
| geometry/marching_squares.rs | cell case table, `chain_segments`, `CHAIN_EPS` | 324 |
| geometry/contour_extract.rs | `weave_contours` (fibers), `marching_squares_bool_grid` | 607 |
| geometry/monotone_cells.rs | boustrophedon cell decomposition, PCA frame, `ELONGATION_GATE` | 573 |
| geometry/edge_distance.rs | `EdgeDistanceField` point-to-boundary distance | 692 |
| geometry/fiber.rs | `Interval`, `Fiber` for push cutter / waterline | 389 |
| geometry/enriched_mesh.rs | `EnrichedMesh` (mesh + BREP face groups), `FaceGroupId(u16)`, `build_enriched_mesh`, `EnrichedMeshError` | 718 |
| geometry/nn_order.rs | nearest-neighbour picker (`Metric`, `NearestPicker`), crate-private | 1145 (prod 470) |
| geometry/point_runs.rs | `split_runs`, `RunTopology` | 295 |
| geometry/arc_util.rs | `linearize_arc(_into)` | 142 |

### trace/ (8.1k lines)
| file | responsibility | lines |
|---|---|---|
| trace/mod.rs | facade | 12 |
| trace/debug_trace.rs | per-op debug record: `ToolpathDebugRecorder`, scope guard (`Drop`), `ToolpathPhaseSink` trait, `TOOLPATH_DEBUG_SCHEMA_VERSION` | 566 |
| trace/semantic_trace.rs | semantic items: `ToolpathSemanticKind` (26), `SemanticKey` (74, typed wire vocabulary), recorder, `enrich_traces`, artifact writer | 1792 |
| trace/toolpath_spans.rs | `Span`, `SpanKind`(10), `SpanPayload`, `AnnotatedToolpath`, `MoveRemap`, `RemapIndex`, invariant check | 2288 |
| trace/transform_provenance.rs | typestate `Transformed<Unreconciled/Reconciled>`, sealed `RemapConsumer`, `ReconcileSet` | 723 |
| trace/produced.rs | `Produced`, the typed side channel of a generator | 131 |
| trace/narrate.rs | agent-facing prose narration of a toolpath and a cut trace | 2550 (prod 2201) |

### gcode/ (7.8k lines)
| file | responsibility | lines |
|---|---|---|
| gcode/mod.rs (in flux) | NOT only a facade: export config types (`ExportError`, `CoolantMode`, `GcodePhase`, `PhaseTool`, `PhaseDrill`), session door `export_gcode_checked`, datum shift, phase arrangement, sim-trace freshness, `project_load_report`, load-policy enforcement, `PostFormat`, `PostConfig`, `replace_rapids_with_feed`; then 1.6k lines of tests | 3230 (prod ~1620) |
| gcode/ir.rs | `Statement` enum, `Program`, `ProgramMetadata` | 137 |
| gcode/program_builder.rs | `Toolpath` inputs to `Program`: `build_single/phased/multi_setup(_with)` | 1704 (prod 849) |
| gcode/modal.rs | `ModalState` (crate-private) | 58 |
| gcode/emitter.rs | `Program` to bytes with a `PostDefinition` | 1032 (prod 480) |
| gcode/post.rs | `PostDefinition` (TOML), newtypes `Rpm/Feedrate/SafeZ`, `Units`, `WcsCode`, 4 shipped posts via `include_str!("../../posts/*.toml")` | 775 |
| gcode/wizard_overlay.rs | `WizardOverlay` per-export overrides, `ToolChangeMode` | 857 (prod 291) |

### export/ (5.7k lines)
| file | responsibility | lines |
|---|---|---|
| export/mod.rs | facade; no re-exports | 14 |
| export/fingerprint.rs | THREE jobs: toolpath fingerprint + diff; SVG summary; the 6-view software composite renderer (stock, toolpath, mesh); stock fingerprint; `ParameterSweepResult` | 2100 (prod ~1800) |
| export/gcode_validator.rs | G-code invariant rules on emitted text; `invariants_for(PostFormat)`; `validate_machine_safety(&ProjectSession)` | 1368 (prod 971) |
| export/viz.rs | SVG preview and three.js HTML pages (three.js from jsdelivr, `export/viz.rs:126`) | 1331 |
| export/ribbon.rs | tube/ribbon mesh and colour ramps | 437 |
| export/material_colour.rs | `MaterialPalette` | 267 |
| export/artifact_io.rs | the one JSON artifact writer (`pub(crate)`) | 147 |

### diagnostics/ (5.9k lines)
| file | responsibility | lines |
|---|---|---|
| diagnostics/mod.rs | model: `Diagnostic`, `DiagnosticId(String)`, `Scope`, `Severity`, `Category`, `Confidence`, `DiagnosticState`, `Source` | 292 |
| diagnostics/diagnose.rs | `diagnose_toolpath_inputs`, `diagnose_project_diagnostics`; `ToolpathDiagnoseInputs` | 149 |
| diagnostics/ids.rs | 100 `pub const &str` rule ids + `ALL` registry | 510 |
| diagnostics/evidence.rs, fix.rs | `DiagnosticEvidence`, `DiagnosticFix` (tagged enums) | 91, 39 |
| diagnostics/supersession.rs | `apply_supersession` reducer | 80 |
| diagnostics/adapters/from_feeds.rs | feeds warnings and heuristic hints | 542 |
| diagnostics/adapters/from_generation.rs | generation findings (`ToolpathStats`) | 904 |
| diagnostics/adapters/from_model_refs.rs | `ref.model_missing` | 144 |
| diagnostics/adapters/from_preconditions.rs | op preconditions (drill, project curve, selection) | 391 |
| diagnostics/adapters/from_project_diagnostics.rs | session `Verdict` to `Diagnostic` | 113 |
| diagnostics/adapters/from_stale_default.rs | `StaleDefault` to `Diagnostic` | 57 |
| diagnostics/adapters/from_static_checks.rs | stepover/depth/feed/height/tool-op checks, `ResolvedHeights` | 719 |
| diagnostics/adapters/from_tool_load.rs | `ToolpathLoadVerdict` to `Diagnostic` | 1066 |
| diagnostics/tests.rs | unit tests (id registry, supersession) | 737 |

### machine/ (5.0k lines)
| file | responsibility | lines |
|---|---|---|
| machine/mod.rs | `MachineProfile`, `SpindleConfig`, `PowerModel`, `RigidityProfile`, `ControllerSettings`, `ControllerFirmware`, presets | 825 |
| machine/head.rs | `MachineHead`, `DustShoe`, `HeadFit`, `ShoeSetting`, `HeadRefusal` (H1) | 581 |
| machine/kinematics.rs | `MachineKinematics`, cycle-time integrator (`digest_moves`, `walk_junctions`, `solve_move`), link/retract rule, AND the GRBL `$$` importer (`GrblImport`, `from_grbl_settings`) | 2294 (prod 1545) |
| machine/kinematic_utilization.rs | machine-confidence instrument, `classify_move`, `analyse_toolpath` | 805 |
| machine/strategy_advisor.rs | `recommend_strategy` (parallel vs spiral) | 430 |

### tool/ (4.4k lines)
| file | responsibility | lines |
|---|---|---|
| tool/mod.rs | `MillingCutter` trait, `CLPoint`, `ChipGeometry`, `EngagementError`, `ToolDefinition` (cutter + shank + deflection), `edge_drop_by_profile` | 1519 (prod 904) |
| tool/flat.rs, ball.rs, bullnose.rs, tapered_ball.rs, vbit.rs | the five cutters: `FlatEndmill`, `BallEndmill`, `BullNoseEndmill`, `TaperedBallEndmill`, `VBitEndmill` | 404, 369, 498, 626, 671 |
| tool/size_units.rs | `SizeUnits` (mm or inch), fraction parser. Display only | 246 |

### io/ (4.0k lines)
| file | responsibility | lines |
|---|---|---|
| io/mod.rs | `load_model_file`, `load_geometry` (one reader), `model_from_geometry`, `brep_face_items`, size guard, extension table | 357 |
| io/dxf_input.rs | DXF to `ItemSet`, open-piece chaining, `$INSUNITS` | 1720 (prod 858) |
| io/svg_input.rs | SVG to `ItemSet` | 584 |
| io/step_input.rs | STEP via `truck` (cargo feature `step`) | 462 |
| io/tool_library.rs | named tool-catalogue TOML store (24 pub fns) | 508 |
| io/machine_library.rs | named machine-profile TOML store | 223 |
| io/named_toml_library.rs | shared name check, path, list, rename; `LibraryError` trait | 80 |

## 2. Public entry points and external callers

Counts are files outside the area that name the item (tests and benches included). Use `rg` again before you rely on one.

| entry point | defined | outside callers (examples) |
|---|---|---|
| `gcode::export_gcode_checked` (the session door) | gcode/mod.rs:376 | ~16 files; `session/compute/export.rs:64`, `crates/rs_cam_cli/src/project.rs:1051` |
| `gcode::export_gcode_phases_with_overlay_checked`, `..multi_setup_with_overlay_checked` | gcode/mod.rs:1192, :1321 | `crates/rs_cam_viz/src/io/export.rs:627,629,929,1016`; `crates/rs_cam_cli/src/job.rs:409,456,478` (job-file door builds its own phases) |
| `gcode::arrange_phases_by_setup`, `ExportPhases` | gcode/mod.rs:523, :492 | `crates/rs_cam_viz/src/io/export.rs:611` |
| `gcode::emit_gcode` | gcode/mod.rs:102 | `crates/rs_cam_cli/src/sweep.rs:340`, `dressup/arcfit.rs:1382` (test) |
| `gcode::project_load_report`, `sim_trace_is_fresh(_for)` | gcode/mod.rs:705, :583, :607 | `session/compute/export.rs:87,141`; `session/load_report.rs:226,370`; `crates/rs_cam_viz/src/state/simulation.rs:336` |
| `gcode::PostFormat` (+`PostConfig`) | gcode/mod.rs:1414, :1510 | ~18 files (session save/load, viz, cli) |
| `gcode::replace_rapids_with_feed` | gcode/mod.rs:1388 | viz only: `crates/rs_cam_viz/src/io/export.rs:643,942,1028` |
| `export::gcode_validator::validate(_with)`, `validate_machine_safety` | export/gcode_validator.rs:232,238,742 | viz only: `crates/rs_cam_viz/src/io/export.rs:106,229,252`, `app/mcp.rs:912`. No core, cli or mcp caller |
| `export::viz::{toolpath_to_svg, toolpath_standalone_3d_html, stacked_simulation_3d_html, stock_mesh_to_3d_html}` | export/viz.rs:284,370,580,1217 | `crates/rs_cam_cli/src/job.rs:492,569,580`, `run.rs:204`; viz `app/mcp/simulation.rs:313`, `app/mcp/view.rs:256` |
| `export::fingerprint::{ToolpathFingerprint, diff_fingerprints, render_*_composite*}` | export/fingerprint.rs:12,142,658-879 | ~25 files; `crates/rs_cam_cli/src/sweep.rs:64,106` |
| `export::artifact_io::write_json_artifact` | export/artifact_io.rs:52 | `stock/simulation_cut/reporting.rs:212` (+ trace) |
| `diagnostics::diagnose_toolpath_inputs` | diagnostics/diagnose.rs:92 | `session/compute/export.rs:184`; `crates/rs_cam_viz/src/ui/properties/operations/validate.rs:457` |
| `diagnostics::apply_supersession` | diagnostics/supersession.rs | called inside `diagnose.rs` only |
| `machine::MachineProfile` | machine/mod.rs:177 | ~131 files |
| `machine::kinematics::compute_cycle_time(_breakdown)` | machine/kinematics.rs:795,1110 | `session/compute.rs:570,643`; `crates/rs_cam_cli/src/nc_replay.rs:162,430` |
| `machine::kinematic_utilization::analyse_toolpath` | machine/kinematic_utilization.rs:519 | `session/compute/diagnostics.rs:618`; `classify_move` also in `dressup/feedopt.rs:232` |
| `machine::strategy_advisor::recommend_strategy` | machine/strategy_advisor.rs:134 | `session/compute.rs:1635` (single caller) |
| `machine::head::{MachineHead, HeadFit, ShoeSetting}` | machine/head.rs | ~24 files |
| `tool::MillingCutter`, `ToolDefinition`, the five cutters | tool/mod.rs:285,625 | ~255 files (the widest dependency in the area). Factory `compute/cutter.rs:8 build_cutter`; second factory `crates/rs_cam_cli/src/job.rs:845 build_tool` |
| `io::load_model_file` | io/mod.rs:138 | `crates/rs_cam_viz/src/io/import.rs:28-57`; mcp, cli; project door is `session/project_file.rs` via `load_geometry` |
| `io::tool_library::*`, `io::machine_library::*` | io/tool_library.rs, io/machine_library.rs | ~8 and ~7 files (viz modals, mcp) |
| `geometry::boundary::{effective_boundary, clip_annotated_to_boundary_set}` | geometry/boundary.rs:36,187 | `session/compute.rs:146`; `session/compute/generation.rs:881,919,1094` |
| `geometry::region_set::RegionSet` | geometry/region_set.rs:29 | ~46 files |
| `geometry::region_mask::region_polygons_from_mask_reported` | geometry/region_mask.rs:169 | `maps/tier_islands.rs:1005,1022`; `compute/execute/standing_material.rs:255` |
| `geometry::enriched_mesh::build_enriched_mesh` | geometry/enriched_mesh.rs:330 | `io/step_input.rs` plus tests |
| `geometry::monotone_cells::*` | geometry/monotone_cells.rs | ~27 files (finish planner) |
| `trace::narrate::narrate_toolpath(_with_context)` | trace/narrate.rs:471,497 | `crates/rs_cam_viz/src/app/mcp/diagnostics.rs:45,151` |
| `trace::debug_trace::*` | trace/debug_trace.rs | ~126 files (generators open scopes) |
| `trace::semantic_trace::*` | trace/semantic_trace.rs | ~52 files; `SemanticKey::` used in `compute/annotate.rs` (89 hits), `compute/execute/dressup_apply.rs` (25) |
| `trace::toolpath_spans::*` (`AnnotatedToolpath`, `SpanKind`) | trace/toolpath_spans.rs | ~178 files |
| `trace::transform_provenance::Transformed` | trace/transform_provenance.rs:331 | ~43 files (every post-generation transform) |
| `trace::produced::Produced` | trace/produced.rs:28 | ~15 files |

## 3. Dependencies and layering

Import counts of `crate::<top>` per folder (production plus inline tests, from `rg`).

| folder | main imports | notes |
|---|---|---|
| geometry | polygon 14, geometry 10, geo 9, trace 6, toolpath 6, mesh 5, surface 3, compute 2 | mostly primitives |
| trace | compute 48 (mostly in narrate tests), trace 44, geo 19, finish 12, toolpath 11, stock 7, dressup 7 | |
| gcode | tool_load 54, compute 18, session 8, ops 7, feeds 6 | mod.rs only |
| export | geo 19, stock 15, trace 10, dexel_stock 8, session 5 | |
| diagnostics | compute 30, feeds 15, tool_load 14, ids 11, session 9 | aggregator; expected |
| machine | machine 30, toolpath 13, feeds 5, gcode 3 | |
| tool | feeds 35, geo 13, machine 5, mesh 4, stock 2 | |
| io | io 22, settings 7, selection 6, geometry 5, compute 3, session 1 | |

Layering inversions and cycles (an inversion = a lower layer imports a higher one):
- gcode/mod.rs imports `session::ProjectSession` (gcode/mod.rs:22), `compute::catalog::effective_spindle_rpm` (:21), `stock::simulation_cut` (:23), `compute::simulate::hash_toolpath` (:~650), `crate::tool_load::*` (about 50 uses), `session::SetupEvalContext` (:353), `ops::drill::DrillCycle` (:165, also `gcode/program_builder.rs:780`). The emitter folder is also the session export orchestrator and the load-gate policy owner.
- gcode and export form a cycle. `gcode/emitter.rs:129` and `gcode/program_builder.rs:484` call `export::gcode_validator::strip_comments`. `export/gcode_validator.rs` imports `gcode::{PostFormat, WizardOverlay}`.
- export/gcode_validator.rs:721 `validate_machine_safety(session: &ProjectSession, ...)` pulls `session::SetupEvalContext`. A text validator reads session state.
- tool/mod.rs imports `compute::tool_config::ToolMaterial` (:23), `feeds::ToolGeometryHint` (trait method `geometry_hint`, tool/mod.rs:585 and each cutter), `machine::head::HeadFit` and `stock::collision::ToolAssembly` (tool/mod.rs:667). The cutter layer knows feeds, machine and stock.
- machine/mod.rs:262 imports `gcode::PostFormat`. machine/strategy_advisor.rs:40 imports `compute::operation_configs::ClearingStrategy`. machine/head.rs:36 imports `stock::collision`. machine/mod.rs:140 imports `feeds::OperationFamily`. A machine-profile cycle with gcode/feeds/compute.
- io/mod.rs:25 imports `session::{LoadedGeometry, LoadedModel, SessionError}` and `compute::stock_config` (:22). io/tool_library.rs:25 imports `compute::tool_config::ToolConfig`. Importers return session types, and the session calls the importers.
- geometry/boundary.rs:188-194 imports `trace::toolpath_spans` and `trace::transform_provenance` (clip returns `Transformed`). geometry/monotone_cells.rs:60 imports `surface::dropcutter`.
- trace/transform_provenance.rs:503,507 imports `finish::scallop::ScallopRuntimeAnnotation`. trace/narrate.rs:199 imports `session::FeedStage`, :67 `compute::catalog::OperationType`.
- diagnostics imports `compute`, `session`, `feeds`, `tool_load` (expected for an aggregator). But `session::ProjectDiagnostics` (session) feeds `diagnostics::adapters::from_project_diagnostics`, and `session` calls `diagnostics`: a two-way link.
- export/viz.rs:126 loads three.js from a CDN URL at page open (no offline use). Not an inversion; a runtime dependency.

## 4. Traits

| trait | defined | implementors / users |
|---|---|---|
| `MillingCutter` (Send+Sync, ~30 methods, 3 SEMANTIC alias pairs: `radius`/`envelope_radius_mm`, `cusp_radius`/`cusp_radius_mm`, `engagement_radius`/`engagement_radius_mm`) | tool/mod.rs:285 | `FlatEndmill` tool/flat.rs:33, `BallEndmill` ball.rs:31, `BullNoseEndmill` bullnose.rs:55, `TaperedBallEndmill` tapered_ball.rs:101, `VBitEndmill` vbit.rs:65, `ToolDefinition` (delegates) tool/mod.rs:802. Used as `Box<dyn MillingCutter>`. Dynamic dispatch; no exhaustive match forces a new cutter to appear anywhere |
| `ToolpathPhaseSink` (Send+Sync) | trace/debug_trace.rs:8 | one implementor, outside the area: `crates/rs_cam_viz/src/compute/worker.rs:1027` (`ToolpathPhaseTracker`) |
| `ReconcileState` (sealed) | trace/transform_provenance.rs:88 | `Unreconciled`, `Reconciled` (typestate markers) |
| `RemapConsumer` (sealed) | trace/transform_provenance.rs:436 | `SemanticLinkChannel` :468, `ScallopAnnotationChannel` :514 |
| `ClipPermit` (`pub(crate)`) | geometry/boundary.rs:353 | `BoundaryPermit` :369; `compute::obstacle::ObstaclePermit` (outside) |
| `LibraryError` (`pub(crate)`) | io/named_toml_library.rs:20 | `MachineLibraryError` io/machine_library.rs:47, `ToolLibraryError` io/tool_library.rs:50 |
| `Drop` guards | trace/debug_trace.rs:482 `ToolpathDebugScope`; trace/semantic_trace.rs:1015 `ToolpathSemanticScope` | RAII scope close |

Traits from elsewhere. The area uses `thiserror::Error` (io, `gcode::post::LoadError`, `EnrichedMeshError`), hand-written `Display + Error` (`ExportError` gcode/mod.rs:50, `HeadRefusal` machine/head.rs:178, `SpanInvariantViolation` trace/toolpath_spans.rs:817, `GridShapeError` geometry/grid2.rs:57), and serde derives. It does NOT define a `Post`, `Importer`, `Adapter` or `Validator` trait. Posts are data (`PostDefinition`), importers are free functions dispatched by `ModelKind` match (io/mod.rs:79), adapters are free functions called one by one in `diagnose.rs:92-141`, validator rules are free functions listed one by one in `validate_with` (export/gcode_validator.rs:238-258). Registration is by hand in each case.

## 5. Recurring patterns

| concern | pattern used here | example | grade | deviations inside the area |
|---|---|---|---|---|
| errors and refusals | Per-door typed errors: `thiserror` enums in io (`DxfError`, `SvgError`, `StepImportError`, `ToolLibraryError`, `MachineLibraryError`), `EnrichedMeshError`, `PostLoadError`. Export refusals are ONE struct `ExportError{message: String}`. Refusal flows as `Err`; "reported" variants carry a failure channel next to the value (`effective_boundary_reported`, `region_polygons_from_mask_reported` + `RegionCapReport`). `unwrap/expect/panic/indexing` are denied by workspace lint | gcode/mod.rs:38 (`ExportError`); geometry/boundary.rs:66 | LINT (panic family) + TYPES (typed enums) for io/geometry; CONVENTION for export | `ExportError` is a message string, so callers cannot match a cause (gcode/mod.rs:38-48, 11 fns return it). `io/mod.rs:79 load_geometry -> Result<_, String>` flattens typed door errors with `format!` (:84-117). `tool/mod.rs:214 EngagementError{reason: String}`. Cutter constructors `assert!` on bad input (tool/bullnose.rs:56-70, tapered_ball.rs:46-51, vbit.rs:36) but `FlatEndmill::new`/`BallEndmill::new` (flat.rs:23, ball.rs:22) take any value. `Mutex::lock().expect("... poisoned")` in trace/debug_trace.rs:221,286,320,354,448,460 and trace/semantic_trace.rs:668,695,728,748,1001. `expect` on shipped post TOML at gcode/post.rs:460-487. `io/svg_input.rs:178 expect("len >= 2")`. `machine/strategy_advisor.rs:173 expect`. `gcode/mod.rs:1608 post_format_or_grbl` degrades an unknown post token to GRBL without a refusal |
| state mutation | The area does not mutate `ProjectSession`. It reads it (`gcode/mod.rs:376`, `export/gcode_validator.rs:721`). Mutation goes through `Command` in `session/`. Pure `&mut` helpers on config values exist | gcode/mod.rs:376 (`&ProjectSession` only) | CONVENTION (session `apply` is outside this area) | `machine/kinematics.rs:243 GrblImport::apply_to(&mut MachineProfile)` mutates a profile directly; called from the session command row and the GUI draft. `PostConfig::apply_controller_options(&mut WizardOverlay)` gcode/mod.rs:1556. `geometry/enriched_mesh.rs:166 apply_uniform_scale(&mut self)` |
| parameters and config | No `OpParams` here. Config types are plain serde structs with `#[serde(default = "fn")]` per field and "no key means default" (`PostConfig`, `MachineProfile`, `MachineKinematics`). Presets are constructor fns (`MachineProfile::presets` machine/mod.rs:579, `MachineHead::preset(name)` head.rs:319). Validation is local: `MachineHead::validate -> HeadRefusal` head.rs:350. Range constants live beside use (`AGGRESSIVENESS_MIN/MAX` machine/mod.rs:425-429) | gcode/mod.rs:1510; machine/mod.rs:177 | CONVENTION | No `validate` on `MachineProfile`, `MachineKinematics`, `PostConfig`, `PostDefinition` (a bad TOML value loads). No `deny_unknown_fields` anywhere in the scope (`rg` finds none), so a misspelt key in a post TOML or machine TOML is ignored. `WizardOverlay::is_empty` lists every field by hand (gcode/wizard_overlay.rs:123-135); a new field that is not added there is silently skipped. Preset names are strings (`MachineHead::PRESETS` head.rs:328; `MachineProfile::from_key` machine/mod.rs:678 falls back to generic) |
| units and coordinate frames | All lengths are mm in bare `f64`. Units live in the field NAME suffix: `_mm`, `_mm_min`, `_mm_s2`, `_deg`, `_s`, `_n`. Frame is a comment/convention: setup-local vs world vs emission frame. Export applies one shift `export_datum_shift_for_toolpath` (gcode/mod.rs:348) and `toolpath_in_export_datum` (:360) | machine/kinematics.rs:93-140; gcode/mod.rs:348 | DOC / CONVENTION | Newtypes `Rpm`, `Feedrate`, `SafeZ` exist (gcode/post.rs:22-52) but only `Rpm` is used, and only in `PostLimits` (wizard_overlay.rs:167). `Statement` uses bare `f64`/`u32` (gcode/ir.rs:22-100). `PostConfig.safe_z: f64`. `Units` (gcode/post.rs:108), `SizeUnits` (tool/size_units.rs:30) and `compute::stock_config::ModelUnits` are three unit enums. No frame type: `Polygon2` boundary has no frame tag (geometry/boundary.rs:36, geometry/CLAUDE.md invariant 1 rests on DOC). Angles: degrees in config (`helix_deg`), radians inside (`arc_engagement_radians` tool/mod.rs:413) |
| identifiers | `ids.rs` newtypes (`ToolpathId`, `ToolId`, `SetupId`, `ModelId`) at the session layer. In-area: `FaceGroupId(u16)` (geometry/enriched_mesh.rs:22), `SpanId(u32)` (trace/toolpath_spans.rs:26), `DiagnosticId(String)` (diagnostics/mod.rs:78) | geometry/enriched_mesh.rs:22 | TYPES for `ToolpathId`, `SpanId`, `FaceGroupId`; NONE for the rest | `GcodePhase`/`PhaseTool.id: usize` is the tool config id (gcode/mod.rs:127) but `ToolId` exists. `Scope::Tool{id: usize}` and `Scope::Setup{id: usize}` (diagnostics/mod.rs:108-118). `ModelRefContext.model_id: usize` (diagnostics/adapters/from_model_refs.rs:42). `export_datum_shift_for_toolpath(toolpath_index: usize)` mixes with ids (gcode/mod.rs:350). Debug and semantic trace ids are bare `u64` (trace/debug_trace.rs, semantic_trace.rs:927). Diagnostic rule id is a `String` wrapper over `&'static str` consts |
| background work, progress, cancellation | None in this area. Functions are synchronous and have no cancel token or progress callback. The one hook is `ToolpathPhaseSink` (trace/debug_trace.rs:8), implemented by the GUI worker | trace/debug_trace.rs:8 | NONE | Heavy kernels with no cancel: `geometry/region_mask.rs:111`, `geometry/monotone_cells.rs:318`, `geometry/boundary.rs:701 model_silhouette`, `geometry/nn_order.rs`, `export/fingerprint.rs:658-879` composite renderers, `io/step_input.rs:47 load_step` |
| caching, freshness, invalidation | Freshness by hash compare. Heads give `fixed_digest` / `effect_digest` (machine/head.rs:399,408) so the session can stale only what read the field. `MachineProfile::link_kinematics` is the compare key (machine/mod.rs:478). `sim_trace_is_fresh(_for)` compares recorded toolpath/config hashes (gcode/mod.rs:583,607). A trace with no provenance block is stale (L7) | machine/head.rs:399; gcode/mod.rs:607 | CONVENTION (+ SENTRY for trace freshness) | Digests are `DefaultHasher` over `format!("{:?}", ..)` of the parts (machine/head.rs:401,410): depends on Debug text and std hasher. The freshness logic is in `gcode/mod.rs`, away from trace/stock. `rewrite_sim_required_to_stale_*` is 4 copies, one per verdict type (gcode/mod.rs:628-690) |
| diagnostics, warnings, user-facing messages | One typed model (`Diagnostic`) plus 8 adapters. Rule ids are `&str` consts in `ids.rs` with `ALL`. Severity/Category/Confidence/State are 4 independent enums. Messages are `format!` strings built in adapters (core holds UI wording). Supersession reducer drops hints when a Current measured finding lists them | diagnostics/mod.rs:249; diagnostics/ids.rs:416 | SENTRY (id uniqueness, id reachable, supersession tests in `diagnostics/tests.rs`); TYPES for the four enums | `ids::ALL` omits 8 consts that adapters use: `GEOM_CLIPPED_BAND`, `GEOM_RAMP_REACH_CLAMP`, `GEOM_OFFSET_LIBRARY_FAILURE`, `GEOM_BOUNDARY_CLIP_DROPPED`, `GEOM_WATERLINE_LEVELS_BELOW_STOCK`, `GEOM_WATERLINE_LADDER_EMPTY`, `CONFIG_DERIVED_STEPOVER`, `CONFIG_CLAIMS_REFERENCE` (checked with a script; `all_diagnostic_ids_are_unique` does not see them). `Diagnostic { .. }` struct literals are written out 51 times in adapters with no constructor (from_static_checks.rs 17, from_generation.rs 14, from_tool_load.rs 9). A second, separate `Severity` in export/gcode_validator.rs:43 and others: `session::VerdictSeverity`, `DrillGateSeverity`. Refusal text names an MCP flag (`accept_unmodeled=true`, gcode/mod.rs:1134) so core copy depends on a wire name |
| serialization and wire types | serde derive with `rename_all = "snake_case"`, `tag = "kind"` for tagged unions, `skip_serializing_if` to keep old files byte-stable. Project file holds `PostConfig` and `MachineProfile` inline. Trace JSON uses typed key enums with exhaustive `as_str` (`SemanticKey` trace/semantic_trace.rs:389; `SpanKind::as_key` toolpath_spans.rs:246). Artifacts stream via `export/artifact_io.rs` | trace/semantic_trace.rs:156; gcode/mod.rs:1414 | TYPES (exhaustive match) + SENTRY (round-trip tests) | `PostFormat` spelling lives in 3 places: serde attrs (gcode/mod.rs:1421-1431), `to_token` (:1467), `from_token` (:1488). `ToolpathSemanticParams.values: BTreeMap<String, serde_json::Value>` (semantic_trace.rs:486) keeps values untyped even with a typed key. `ToolpathDebugSpan.kind: String` (trace/debug_trace.rs:64,78,152) and the consumer compares `s.kind == "z_level_clear"` (trace/narrate.rs:1678). Item lookup by label equality (semantic_trace.rs:1480,1550,1653; narrate.rs:963) |
| numeric tolerances and epsilons | Local `const` per file, no shared module: `CHAIN_EPS` 1e-6 (geometry/marching_squares.rs:131), `CLIP_PARAMETER_EPSILON` 1e-9 (geometry/boundary.rs:558), `CLAMP_EPS` 0.001 (geometry/monotone_cells.rs:325,425), `CLOSE_TOL_MM` 0.01 (io/dxf_input.rs:589), `CIRCLE_LIKE_*` (io/svg_input.rs:260,263), `EPS` 1e-6 (export/gcode_validator.rs:743), `EPS` 1e-9 (gcode/program_builder.rs:687), `DEPTH_BEYOND_STOCK_EPS_MM` (diagnostics/adapters/from_static_checks.rs:507), `Z_EPSILON_MM` 0.05 (trace/narrate.rs:20) | geometry/marching_squares.rs:131 | CONVENTION | Inline `1e-N` literals: about 56 in kinematics.rs, 53 tool/mod.rs, 50 boundary.rs, 35 fiber.rs, 31 vbit.rs (counts include tests). Different files use 1e-6, 1e-9, 1e-10, 1e-12 for the same "equal" test. No literal-source rule for a number (operator rule is DOC in root CLAUDE.md) |
| geometry primitives | Crate-root primitives `geo::{P2,P3,Triangle}`, `polygon::Polygon2`, `mesh::TriangleMesh`. Row-major `Grid2<T>`. Owned `Vec<P3>` runs. `RegionSet<'a>` uses `Cow` | geometry/region_set.rs:29; geometry/grid2.rs:75 | CONVENTION | `Grid2` has few adopters; `grid_field.rs:79,128,181` and `contour_extract.rs:241` take raw slices + `rows, cols`; `maps::grid::GridSpec` is a second grid type (geometry/grid2.rs:20 says "complements"). Five other grids are "intended future adopters" (grid2.rs:16-18). Contours come back as `Vec<Vec<P3>>` (`weave_contours`) or `Vec<Vec<P2>>` (`chain_segments`) with no type. Point-to-segment distance exists as `geo.rs:393` and again in `polygon.rs:1946`, `dressup/link.rs:49`, `finish/direction_field.rs:1701`, `compute/execute/standing_material.rs:764`, `compute/alignment_pins/walls.rs:181` (outside the area, but the area should use `geo`) |
| test style and sentries | Inline `#[cfg(test)] mod tests` with `#![allow(clippy::unwrap_used, ...)]`; folder `tests.rs` (diagnostics); integration tests in `crates/rs_cam_core/tests/` named after a defect id (`_f1`, `_g_xxx`, `_p2_1`). Each folder CLAUDE.md names sentries. All 40 sentry test files named in the 8 CLAUDE.md files exist. Test code is often as long as production code | gcode/mod.rs:1621; diagnostics/tests.rs:1 | SENTRY | `gcode/mod.rs` has ~1600 test lines in the facade file. `diagnostics/tests.rs:5` blanket `#![allow(`. `export/viz.rs` (1331 lines), `diagnostics/adapters/from_tool_load.rs` (1066), `from_static_checks.rs` (719) and `machine/kinematic_utilization.rs` (805) hold no inline tests (tests are elsewhere or absent; not verified) |
| naming and module layout | One folder per concern, facade `mod.rs` with `pub mod` list; adapters named `from_<source>.rs`; unit-suffixed fields; defect-id prefixes in comments (`G7:`, `W2.5`, `P2.4`). Variant suffix families: plain / `_reported` / `_with_overlay` / `_with_provenance` / `_checked` / `_with` | geometry/mod.rs; gcode/program_builder.rs:73-235 | CONVENTION | API proliferation: `clip_toolpath_to_boundary` + `_with_provenance` + `_set_with_provenance` + `clip_annotated_to_boundary_set` (geometry/boundary.rs:164,187,271,332); `region_polygons_from_mask` + `_clamped` + `_reported` (region_mask.rs:111,150,169); `export_gcode_phases_checked` and `export_gcode_multi_setup_checked` (no overlay) have only test callers (gcode/mod.rs:1169,1302). `gcode::emit_program` (mod.rs:110) re-exports `emitter::emit_program` (emitter.rs:22) with a "backward-compatible" doc against the no-legacy ruling. Files named for one thing but holding more: `export/fingerprint.rs`, `gcode/mod.rs`, `machine/kinematics.rs` (GRBL importer), `geometry/boundary.rs` (silhouette) |
| registration of a new feature | Enum variant + exhaustive match arms + manual `ALL` array + a data file or id const. See section 6 | gcode/mod.rs:1414-1500 | TYPES for the matches; CONVENTION or SENTRY for `ALL` lists | `PostFormat::ALL` (gcode/mod.rs:1435) and `SemanticKey::ALL` (semantic_trace.rs:309) are written by hand. For `SemanticKey` and `ToolpathSemanticKind` the `ordinal()` exhaustive match is the proof that `ALL` is complete (semantic_trace.rs:90-120); `PostFormat::ALL` has no such proof. `diagnostics::ids::ALL` has the gap above |

## 6. Extension points

| to add | files and match arms | compiler forces | not forced |
|---|---|---|---|
| a post dialect (e.g. a new controller) | `gcode/mod.rs:1414` `PostFormat` variant (+ serde `rename`/`alias`); `PostFormat::ALL` :1435; `label` :1443; `definition` :1453; `to_token` :1467; `from_token` :1488; `gcode/post.rs:443-487` new `const X_TOML = include_str!` and accessor `fn x() -> &'static PostDefinition`; new `crates/rs_cam_core/posts/<x>.toml`; `export/gcode_validator.rs:137 invariants_for` arm; `machine/mod.rs:262 ControllerFirmware::post_format` if it has a firmware; viz dropdown and CLI flag read `PostFormat::ALL` | `label`, `definition`, `to_token`, `invariants_for` matches (exhaustive, no wildcard) | `PostFormat::ALL` entry; `from_token` string; serde attrs; the TOML content; the `ControllerFirmware` map |
| a post field (data) | `gcode/post.rs` `PostDefinition` field with `#[serde(default)]`; all four TOML files; `emitter.rs` reads it | none (new field with default compiles) | TOML coverage in the 4 files; `deny_unknown_fields` absent so a typo passes |
| a `Statement` kind (new G-code word) | `gcode/ir.rs:22` variant; `gcode/emitter.rs` match (render); `gcode/wizard_overlay.rs:200,266,281` (`apply_to_program`, `clamp_dry_run_z`: has a wildcard arm at :281 area); `program_builder.rs` `commanded_z` :453 (`max_commanded_z`); `export/gcode_validator.rs` rules if the word is new | emitter match (if no wildcard) | overlay wildcard arms and `commanded_z` can miss the new variant silently |
| a wizard override | `gcode/wizard_overlay.rs:72` field; `is_empty` :123; `applied_post` :145 or `apply_to_program` :200; viz wizard UI; `PostConfig::apply_controller_options` gcode/mod.rs:1556 if it comes from the machine profile | none | `is_empty`, `applied_post` / `apply_to_program` use of the field |
| a cutter shape | new `tool/<shape>.rs` implementing `MillingCutter` (required: `diameter`, `length`, `chip_geometry`, `height_at_radius`, `width_at_height`, `geometry_hint`, drop-cutter `vertex/facet/edge_drop`, ...); `tool/mod.rs` `pub use`; `feeds::ToolGeometryHint` variant (feeds/mod.rs:75) and every match on it (feeds, `tool/mod.rs:331,396,774`); `compute/tool_config.rs:13 ToolType` variant and its ~18 match arms; `compute/cutter.rs:8 build_cutter`; `crates/rs_cam_cli/src/job.rs:845 build_tool`; viz tool panel (`crates/rs_cam_viz/src/ui/properties/tool.rs`, `tool_library_modal.rs`); `compute/tool_config.rs` serde tokens | `ToolType` and `ToolGeometryHint` matches | the `MillingCutter` impl is only forced when someone builds the new `Box`; `build_tool` in the CLI is a second factory; `stock/radial_profile.rs:165-185` hand list of test cutters |
| a machine-profile field | `machine/mod.rs:177` field with `#[serde(default)]`; preset constructors :487-579; viz machine panel; `ProjectSession` machine command row (`session/command.rs`); if it changes generation, add it to `link_kinematics` (:478) or to `MachineHead::fixed_digest` so the right results stale; `session/mutation/config.rs` compare | struct literal sites (presets) when the field has no `..Default` | the stale-set logic (digests / compare) has no test that a new field is covered |
| a head part or shoe preset | `machine/head.rs` `MachineHead::preset` :319, `PRESETS` :328, `validate` :350, `bands` :384, digests :399-408 | none | `PRESETS` strings vs `preset` match arms |
| a diagnostic rule | `diagnostics/ids.rs` const + entry in `ALL` (:416); an adapter fn that builds `Diagnostic { .. }` (id, scope, category, severity, confidence, state, source, message, evidence, fix, supersedes); call it from `diagnose.rs:92-141` (or from `session/compute/export.rs` for project scope); add `supersedes` ids if a measured rule covers a hint; row in `diagnostics/tests.rs` id list (:100-) | none (ids are strings) | `ALL` entry (8 already missing), the orchestrator call, the id test list, the supersedes link |
| a diagnostic source adapter | new `adapters/from_<x>.rs`, `adapters/mod.rs` `pub mod`, `ToolpathDiagnoseInputs` field (diagnose.rs:36-78), call in `diagnose_toolpath_inputs`, every constructor of `ToolpathDiagnoseInputs` (`session/compute/export.rs:~160`, `crates/rs_cam_viz/.../validate.rs:~440`) | struct literal completeness at the 2 call sites | the producer call order; `Source` enum variant |
| a `Source`/`Category`/`Severity` variant | `diagnostics/mod.rs`; `Severity::rank` match; GUI badge and MCP serializer | `rank` match (exhaustive) | UI consumers outside the area |
| an import door (new file kind) | `compute/stock_config.rs` `ModelKind` variant; `io/mod.rs:79 load_geometry` arm; `io/mod.rs:293 MODEL_FILE_EXTENSIONS` and `infer_kind_from_path` :298; new `io/<x>_input.rs` with its own error enum and `file_size_over_limit` call; `session/project_file.rs` (the project door reads through `load_geometry`); `Cargo.toml` feature gate if optional (pattern of `step`); viz file dialog | `ModelKind` match in `load_geometry` | extension table; the size guard (sentry `every_import_door_refuses_an_oversized_file_edg03` covers three doors by name) |
| a named TOML library kind | implement `LibraryError` for a new error enum; use `named_toml_library::{path_in, list_in, rename_in}`; `settings::paths` folder | trait impl | name-safety test per library |
| a span kind | `trace/toolpath_spans.rs:154` variant; `SpanKind::ALL` :224 (length literal); `as_key` :246; `label` :274; `classify_span_path` / transit lists (:580-692, `tool_load::locality::is_phantom_transit`); `export/ribbon.rs` via `classify_span_path`; `compute/annotate.rs` producers | `as_key`, `label`, `SpanClass` mapping (exhaustive) | the `ALL` length is a literal that stops a miss only when the count changes; transit/phantom lists in `tool_load` |
| a semantic kind / param key | `trace/semantic_trace.rs:16` `ToolpathSemanticKind` + `ALL`:60 + `ordinal`:97; `SemanticKey` :156 + `ALL`:309 + `as_str`:389; producers call `scope.set_param(SemanticKey::X, v)` | `ordinal`/`as_str` matches; `ALL` length | value type of a key (untyped `Value`) |
| a post-generation transform that moves move indices | return `Transformed<Unreconciled>` (trace/transform_provenance.rs:331); caller builds a `ReconcileSet` (:547) with the channels it owns, calls `.reconcile(...)`, then `.into_inner()` | compile error to skip reconcile (`into_inner` absent on Unreconciled, `#[must_use]`) | a NEW index-carrying channel must be added to `ReconcileSet`; a new channel kind is a new sealed impl in this file only |
| a validator rule | `export/gcode_validator.rs` `FindingKind` variant (:54), `rule_*` fn, call in `validate_with` (:238-258), `PostInvariants` field and each `invariants_for` arm | `invariants_for` arms (struct literal) | the call line in `validate_with`; CLI/MCP do not run the validator at all |
| a G-code export door | none exists as a single trait. Doors: `export_gcode_checked` (session), viz `io/export.rs` x3, cli `job.rs` x3, each builds `GcodePhase` by hand | none | parity of datum shift, coolant, feeds line, `high_feedrate_mode`, validator (see section 9) |
| a preview output format | `export/viz.rs` fn or `export/fingerprint.rs` renderer; wire into cli/viz callers | none | |
| an artifact JSON dump | route through `export/artifact_io.rs:52 write_json_artifact` (`pub(crate)`) | visibility | streaming rule is DOC |

## 7. CLAUDE.md drift

| file | statement | reality |
|---|---|---|
| tool/CLAUDE.md:10-11 | files hold `CylCutter`, `BallCutter`, toroidal `BullCutter`, `BallConeCutter` | types are `FlatEndmill`, `BallEndmill`, `BullNoseEndmill`, `TaperedBallEndmill`, `VBitEndmill` (tool/mod.rs:13-17). The old names remain only in file-header doc comments (tool/flat.rs:1, bullnose.rs:1, tapered_ball.rs:1) |
| tool/CLAUDE.md:7 | `mod.rs` holds "the cutter trait, `cusp_radius`, `cusp_radius_mm`, `valley_radius_mm`, `edge_drop_by_profile`" | also holds `ToolDefinition` (tool/mod.rs:625), `CLPoint`, `ChipGeometry`, `EngagementError`, deflection model :703; not listed |
| gcode/CLAUDE.md:11 | `mod.rs` — "the facade and the export configuration" | `gcode/mod.rs` is 3230 lines: session export door, datum shift, phase arrangement, sim-trace freshness, `project_load_report`, exceed/unmodeled policy, `PostFormat`, `PostConfig`, high-feedrate rewrite. No file map line for them |
| gcode/CLAUDE.md:8 | "`gcode::emitter::emit_program`" is the emit call | `gcode::emit_program` also exists (gcode/mod.rs:110) as a re-export with a "backward-compatible" comment; not mentioned |
| gcode/CLAUDE.md:22-23 | "A post-processor is data, not code. Add a `PostDefinition` row, not a branch in the emitter" | A new dialect also needs `PostFormat` variant plus 5 match/array sites and a validator arm (section 6). Not "a row" |
| export/CLAUDE.md:3-4 | "The entry point is `export::mod`, which re-exports the four surfaces" | `export/mod.rs` has only `pub mod` lines, no re-export (export/mod.rs:8-14); six modules, not four |
| export/CLAUDE.md:7-12 | file map lists `viz`, `ribbon`, `material_colour`, `fingerprint`, `gcode_validator`, `artifact_io` | `fingerprint.rs` is described as "toolpath fingerprinting and diffing" but holds the 6-view composite renderer (:658-1009) and `StockFingerprint` (:1693) too. The "6-view composite" invariant (line 20) names no file |
| export/CLAUDE.md invariant 1 | "The datum rule for Z zero lives in `../gcode/CLAUDE.md`" | OK. But the validator takes the datum from `SetupEvalContext` (export/gcode_validator.rs:721-725) |
| diagnostics/CLAUDE.md:8-9 | `diagnose.rs` — the orchestrator | OK. `diagnostics/mod.rs` module docs are stale: "PR-2 landing (this commit) is plumbing only", "[`diagnose_toolpath`] (PR-3)" (diagnostics/mod.rs:34-44,247-248). Real fns are `diagnose_toolpath_inputs` and `ProjectSession::diagnose_toolpath` (`session/compute/export.rs:101`) |
| diagnostics/CLAUDE.md:21-22 | "A new rule needs an identifier in `ids.rs`" | True, but `ids::ALL` is incomplete (8 consts, section 5) and nothing fails |
| diagnostics/CLAUDE.md:33 | sentry `--lib the_static_check_and_the_generator_refuse_the_same_tools` | test lives at `compute/execute/tests.rs:1221`, outside the folder; exists |
| machine/CLAUDE.md:25-26 | "The cycle time and the predicted feed come from ONE pass: `digest_moves` then `walk_junctions` ... Do not add a second digest loop" | `digest_moves` + `walk_junctions` run in two places: `compute_cycle_time_breakdown` (kinematics.rs:1124,1130) and `predicted_feeds_for_toolpath` (:1238,1243). Shared fns, two passes. The wording is not wrong but is not what is true |
| machine/CLAUDE.md:7-8 | `kinematics.rs` — "the kinematics model for cycle-time estimation" | also holds the whole GRBL `$$` importer (`GrblImport`, `from_grbl_settings`, `detect_firmware`, kinematics.rs:179-620), about 450 lines of controller parsing |
| machine/CLAUDE.md | does not list `MachineProfile::from_key` as test-only | `from_key` is a test door (machine/mod.rs:678 doc) |
| geometry/CLAUDE.md:19-20 | `ToolContainment::Inside` "can fall back to unclipped geometry" and "Inspect `boundary_clip_dropped`" | the code comment (geometry/boundary.rs:27-33) says "every production call site" uses `effective_boundary_reported`, but `session/compute.rs:146` calls plain `effective_boundary` and drops the failure channel (the comment at session/compute.rs:121-122 says the clip refuses on the same failure) |
| geometry/CLAUDE.md:8 | `boundary.rs` — "the machining boundary and `ToolContainment`" | also holds toolpath clipping (4 variants, `clip_walk`, `ClipPermit`, :164-567) and model silhouette (:677-760) |
| geometry/CLAUDE.md:34 | "Do not copy a grid walk. Use `grid2.rs` or `maps/grid.rs`." | no enforcer; `grid_field.rs` and `contour_extract.rs:241` take raw slices |
| trace/CLAUDE.md invariants | "Do not add a `match` on the op type here" (narrate) | holds: `narrate.rs:1082-1084,1239,1294,1426` read `capabilities()`. The only `OperationType::` literal is a test (narrate.rs:2269) |
| trace/CLAUDE.md:7-8 | "`semantic_trace.rs` — the semantic items for operation-level diagnostics" | also owns `SemanticKey` vocabulary, `enrich_traces` (:1022) and the artifact writer (:1240) |
| io/CLAUDE.md:9-11 | `mod.rs` — "import helpers, size guard, `brep_face_items`" | also owns `load_geometry` (the one reader, io/mod.rs:75), `model_from_geometry`, `infer_kind_from_path`, extension table |
| io/CLAUDE.md:26-27 | "Every importer refuses a file over `MAX_IMPORT_FILE_SIZE`" | true for the 3 doors; `TriangleMesh::from_stl_scaled` (STL, io/mod.rs:81-82) is called with no size guard in this file (not verified in `mesh.rs`) |
| root CLAUDE.md "folder-level files ... each 40 lines or fewer" | | all 8 files are 39-40 lines |

## 8. Navigability

| item | evidence |
|---|---|
| Files over ~1500 lines | gcode/mod.rs 3230 (in flux); trace/narrate.rs 2550; machine/kinematics.rs 2294; trace/toolpath_spans.rs 2288; export/fingerprint.rs 2100; trace/semantic_trace.rs 1792; geometry/boundary.rs 1746; io/dxf_input.rs 1720; gcode/program_builder.rs 1704; tool/mod.rs 1519. About half of several of these is inline tests |
| Mixed responsibilities | gcode/mod.rs (export orchestration + policy + freshness + load report + post config); export/fingerprint.rs (fingerprint + SVG summary + software renderer + stock fingerprint + sweep result); machine/kinematics.rs (integrator + GRBL importer + presets); geometry/boundary.rs (boundary + clip + silhouette); tool/mod.rs (trait + deflection beam model + `ToolDefinition`); trace/toolpath_spans.rs (span types + `AnnotatedToolpath` + `MoveRemap` + `RemapIndex`) |
| Items in the wrong folder | `project_load_report`, `enforce_*_policy`, `sim_trace_is_fresh(_for)`, `rewrite_sim_required_to_stale_*` (gcode/mod.rs:583-1160) belong with `tool_load/` or `session/`. `GrblImport` (machine/kinematics.rs:179-620) belongs with `io/` or a `machine/controller` file. `PostConfig` (gcode/mod.rs:1510) is a project-file type, not G-code. `strip_comments` (export/gcode_validator.rs:263) is a G-code lexer helper used by gcode. `export/ribbon.rs` and `export/material_colour.rs` are GUI mesh colouring in `export/`. `trace/narrate.rs` (prose for an LLM) is an agent-surface module in `trace/`. `ToolContainment` (geometry/boundary.rs:15) duplicates `compute::config::BoundaryContainment` (mapped by hand at `session/compute.rs:134-138`) |
| Misleading names | `gcode::emit_gcode_phased` is test-only (gcode/mod.rs:205-210). `export::viz` is a preview writer, not the GUI viz. `diagnostics::Source` vs `Category` vs `Scope`: "scope" here is a target, but `session::Scope` (generation plan) exists with the same name. `export_gcode_checked` (session door) vs `export_gcode_phases_checked` (test-only) vs `..._with_overlay_checked` (the real phase door) |
| Dead or retired in place | `FindingKind::WrongProgramEndCode` and `MissingProgramBrackets` never fire (export/gcode_validator.rs:~60-68; "variant kept for downstream matches") yet `Finding` is not serialised; `MachineHead::holder_diameter_mm` has no caller (machine/head.rs:334); `Scope::Setup` / `Scope::MoveRange` have no constructor in core (diagnostics/mod.rs:108,119; only a pattern at `stock/sim_triage.rs:445` — verify the type there) |
| Instrument false positives | `instruments/dead_pub_surface.md` lists `enforce_unmodeled_policy` (gcode/mod.rs:1077) and `MoveKinematics` (kinematics.rs:636) as unreferenced. Both are used in their own file (gcode/mod.rs:1157; `move_kinematics` returns it). Do not act on those two rows |

## 9. Candidate debt leads (UNVERIFIED)

Ordered by estimated value. "gap" means two surfaces differ.

1. gcode/mod.rs:1388 + `crates/rs_cam_viz/src/io/export.rs:643,942,1028`: `PostConfig.high_feedrate_mode` is applied by the GUI export only. `export_gcode_checked` (gcode/mod.rs:376, used by MCP and `crates/rs_cam_cli/src/project.rs:1051`) never calls `replace_rapids_with_feed`. GUI, MCP and CLI emit different bytes for the same project (root rule: surface number parity).
2. export/gcode_validator.rs:232-258: the G-code validator is called only from viz (`crates/rs_cam_viz/src/io/export.rs:106,229,252`, `app/mcp.rs:912`). The CLI, `export_gcode_checked` and the MCP export skip it. The doc calls it the "safety net under the data-driven post".
3. Three or more export doors assemble `GcodePhase` by hand: gcode/mod.rs:376-480, `crates/rs_cam_viz/src/io/export.rs:611,929,1016`, `crates/rs_cam_cli/src/job.rs:409-478`. The datum, coolant, drill, feed-stage and `pre_gcode` fields can drift (P0-D1 was exactly this class, gcode/mod.rs comment at the `coolant:` line).
4. diagnostics/ids.rs:416: `ALL` omits 8 used constants (list in section 5). The uniqueness test cannot see a duplicate among them.
5. diagnostics/mod.rs:78 `DiagnosticId(String)` and `supersedes: Vec<DiagnosticId>`: a misspelt id in `supersedes` compiles and silently never suppresses. A `&'static str` newtype or an enum would let the compiler check it. `Diagnostic { .. }` is written 51 times with 11 fields each (adapters), no constructor.
6. gcode/mod.rs:1608 `post_format_or_grbl`: an unknown post token in a project file becomes GRBL with no finding. A GRBL program on a LinuxCNC machine is a safety-relevant silent fallback (the comment calls it deliberate).
7. gcode/ir.rs:48 `Statement::Raw(String)` carries modal M-codes written as text (coolant, spindle, G40/G41/G42) so the IR cannot be checked; the emitter and the wizard overlay re-parse text (gcode/emitter.rs:121-140 `unsupported_mcode_in_line`, gcode/program_builder.rs:481-525 `dangerous_modal_words`, plus the validator's word scanner). Three G-code word scanners.
8. gcode/post.rs:22-52: `Rpm`, `Feedrate`, `SafeZ` newtypes exist; `Feedrate` and `SafeZ` have no use outside post.rs/mod.rs re-export, and `Statement` uses bare `f64`. The comment cites "unit mixing has killed real machines".
9. gcode/wizard_overlay.rs:123: `is_empty` is a hand list of 11 fields. Destructure `Self { .. }` or derive `PartialEq` against `Default` to make the compiler check it.
10. machine/head.rs:399-411 digests via `format!("{:?}")` + `DefaultHasher`: stale-set depends on Debug text and hasher choice; a Debug change alters the digest (only matters across processes if persisted: check `session/` for persistence).
11. gcode/mod.rs:628-690: four near-identical `rewrite_sim_required_to_stale_*` fns, one per verdict type (added one at a time: T-21 comment says depth was missed once). A trait or a generic would make a fifth gate impossible to forget.
12. tool/mod.rs:285 `MillingCutter` has duplicate-meaning accessor pairs ("documented aliases", tool/mod.rs:~270-300) and a 30-method surface; `radius()` / `envelope_radius_mm()` etc. return the same value. Two names for one number invites drift (test `tool_scale_semantics_pr2` pins it).
13. `crates/rs_cam_cli/src/job.rs:845 build_tool` repeats `compute/cutter.rs:8 build_cutter` with its own defaults (`DEFAULT_CUTTING_LENGTH_FACTOR`, flute count 2, carbide). Two cutter factories.
14. tool/flat.rs:23 and ball.rs:22 accept any diameter; bullnose/tapered/vbit assert (panic) on bad input (tool/bullnose.rs:56-70, tapered_ball.rs:46-51, vbit.rs:36). `assert!` is outside the denied clippy set. Viz guards against it by comment (`crates/rs_cam_viz/src/ui/properties/tool.rs:150`).
15. geometry/boundary.rs:15 `ToolContainment` vs `compute::config::BoundaryContainment`: parallel enums with a hand map (`session/compute.rs:134-138`). Same for `gcode::Units` / `SizeUnits` / `ModelUnits`.
16. geometry/region_mask.rs:111-169: 3 variants of one function, where the plain form hides the cap (`MAX_REST_REGIONS`=64) and `_clamped` hides the report. Callers: `finish/finish_planner.rs:537` uses `_clamped`. Check each caller reports a truncation to the user.
17. geometry/boundary.rs:164 `clip_toolpath_to_boundary` and `clip_toolpath_to_boundary_with_provenance` (:271): wrappers whose callers are tests (`direction_field_wanaka_f1.rs:1024`, etc.) — test-only pub API; the safe path returns `Transformed`.
18. gcode/mod.rs:1169, :1302: `export_gcode_phases_checked`, `export_gcode_multi_setup_checked` have only test callers; pub test shims.
19. gcode/mod.rs:110 `emit_program` is a "backward-compatible" re-export; `gcode/mod.rs:102 emit_gcode` used only by `crates/rs_cam_cli/src/sweep.rs:340` and arcfit tests. The no-legacy ruling says delete.
20. export/gcode_validator.rs:58-68 retired rules `WrongProgramEndCode`, `MissingProgramBrackets`, and `PostInvariants.requires_percent_brackets` (all posts `false`) — dead branches.
21. machine/kinematics.rs:179-620: GRBL `$$` parser with a hard-coded `KNOWN: [u32; 15]` list (kinematics.rs:460); the ignored-count shown to the user depends on it, and `grblHAL` extras ($340, $341...) are mixed with GRBL.
22. machine/mod.rs:333 `TOOL_CHANGE_MODE_IGNORE: u32 = 4` and `ControllerSettings` use raw `u32` for grblHAL `$341` (machine/mod.rs:276-335). An enum would stop `4` repeating (`gcode/mod.rs:~270` message text says `$341=4` too).
23. trace/debug_trace.rs:64,78,152 `kind: String` consumed by string compare (trace/narrate.rs:1678 `"z_level_clear"`); producers elsewhere (`rg '"z_level_clear"'`). Typed enum like `SemanticKey` would fix.
24. trace/semantic_trace.rs:486 `values: BTreeMap<String, Value>`: a typed key with an untyped value; each key has one value type by convention only. `insert<T: Serialize>` swallows a serialise error (`if let Ok(value) = serde_json::to_value(value)`, :504).
25. trace/semantic_trace.rs:668-1001 and debug_trace.rs:221-460: `Mutex::lock().expect("poisoned")` x11; a panic in one generator thread poisons the recorder and every later call panics. `Cargo.toml` says panics must unwind (root `Cargo.toml:84`), so this path is reachable.
26. diagnostics/mod.rs:106-125: `Scope::Setup{id: usize}`, `Scope::Tool{id: usize}`, `Scope::MoveRange` — raw `usize` where `SetupId`/`ToolId` exist; `Setup` and `MoveRange` may have no constructor.
27. diagnostics/adapters/from_static_checks.rs:507 + other files: epsilons and thresholds in adapters (`DEPTH_BEYOND_STOCK_EPS_MM`) while the same facts live in `tool_load`/`feeds`. Check for two thresholds for one fact (e.g. "stepover over 80 percent" in static checks vs feeds clamps).
28. diagnostics user text embeds wire/flag names (`accept_unmodeled=true`, gcode/mod.rs:1134-1140): core refusal text depends on MCP parameter names.
29. export/fingerprint.rs:658-1009: a software 6-view renderer in a "fingerprint" file, with `render_mesh_composite` test-only (instrument `test_only_pub_api.md`: 7 test refs). Candidate to split: `fingerprint`, `composite`.
30. export/viz.rs:126: three.js pinned to a CDN URL; the HTML previews need network access. Check whether a user on a shop machine without internet opens them (MCP `screenshot_*` tools use a different renderer).
31. geometry/grid2.rs:16-18: `SurfaceHeightmap`, `SlopeMap`, `DropCutterGrid`, `MaterialGrid` are "intended future adopters" and `grid_field.rs` takes raw slices; the CLAUDE.md "Do not copy a grid walk" rule has no enforcer. Grep for `row * cols + col` / `r * nx + c` in `surface/`, `adaptive/`, `finish/` to size it.
32. geometry/enriched_mesh.rs:22 `FaceGroupId(u16)` with `MAX_FACE_GROUPS = u16::MAX + 1` (:303): a STEP with more faces refuses; check `io/step_input.rs` error path names the limit.
33. io/mod.rs:79-117: `load_geometry` returns `String` errors made by `format!("STL load failed: {e}")`; the typed per-door errors are lost, and `SessionError::ModelLoad{detail}` cannot be matched by cause.
34. io/mod.rs:81: STL load has no explicit `MAX_IMPORT_FILE_SIZE` guard in this file (the sentry names "three doors"); check `mesh.rs` `from_stl_scaled`.
35. io/tool_library.rs: 24 pub fns; every operation has a path-taking `_in(dir, ..)` and an ambient-folder twin (`load_from`/`load_library`, `save_to`/..., `delete_from`/`delete_library`). Pairs double the surface; ambient ones read `settings::paths` global state.
36. machine/mod.rs:466 `effective_kinematics()` and `kinematics: Option<MachineKinematics>` "absence IS the feature flag" (machine/kinematics.rs doc): `None` and `Some(default)` differ in behaviour; every consumer must know it. `max_feed_mm_min` is documented as TRAVEL rate but its name reads like a cutting cap (machine/mod.rs:184-196).
37. machine/mod.rs:697 `matching_preset_index` compares `serde_json::to_string` of two profiles: relies on field order and float formatting.
38. trace/toolpath_spans.rs:458 `AnnotatedToolpath` carries `toolpath` and `spans` as pub fields that must stay in step; `check_invariants` (:765) is a runtime check, called from tests and generation. A private-field + constructor form would make the invariant a type.
39. trace/narrate.rs (2551 lines) mixes formatting with metric thresholds (`LARGE_ARC_RADIUS_MULTIPLIER`, `Z_EPSILON_MM` :20,37); the same thresholds may exist in `tool_load`/`diagnostics`. Check for a second air-cut threshold (narrate.rs:43 points at `Capabilities::air_cut_high_threshold_pct`, good).
40. gcode/mod.rs:205-210 `#[cfg(test)] fn emit_gcode_phased` shim and three `#[cfg(test)]` blocks inside a 3230-line facade — large inline test module in the file under heaviest edit by other sessions (in flux). Review only after those sessions merge.
