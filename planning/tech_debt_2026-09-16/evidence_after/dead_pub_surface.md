# Dead public surface (mechanical)

Definitions: 4999 pub items in production code. Word-count instrument over crates/*/{src,tests,benches,examples}; a name that occurs once in the whole workspace is its own definition and nothing else. Trait-impl methods and macro-generated uses are blind spots: VERIFY each row with rg before acting.

## Zero references anywhere (8)

| kind | name | file:line | vis |
|---|---|---|---|
| const | `TIP_FLOAT_RESOLUTION` | `crates/rs_cam_core/src/compute/config.rs:1449` | pub |
| fn | `load_dir` | `crates/rs_cam_core/src/feeds/vendor_lut.rs:398` | pub |
| fn | `append_toolpath` | `crates/rs_cam_core/src/semantic_trace.rs:947` | pub |
| fn | `evaluate_project` | `crates/rs_cam_core/src/tool_load/mod.rs:567` | pub |
| fn | `active_axes` | `crates/rs_cam_core/src/tool_load/optimize/axes.rs:228` | pub |
| const | `COLLISION_POINT` | `crates/rs_cam_viz/src/render/colors.rs:41` | pub |
| fn | `row_hover_tint` | `crates/rs_cam_viz/src/ui/components/kv_row.rs:250` | pub |
| const | `SPACE_0` | `crates/rs_cam_viz/src/ui/tokens.rs:36` | pub |

## `pub` but referenced only inside its own file (55) — visibility too wide, or dead behind a same-file caller

| kind | name | file:line |
|---|---|---|
| struct | `ParamHint` | `crates/rs_cam_core/src/compute/catalog.rs:1214` |
| struct | `OperationParamSchema` | `crates/rs_cam_core/src/compute/catalog.rs:1229` |
| struct | `NewDefaultCtx` | `crates/rs_cam_core/src/compute/catalog.rs:2679` |
| fn | `floating_fraction` | `crates/rs_cam_core/src/compute/config.rs:1379` |
| struct | `ReferenceEngagement` | `crates/rs_cam_core/src/dressup.rs:2691` |
| struct | `OutsideRegionChord` | `crates/rs_cam_core/src/entry_audit.rs:142` |
| fn | `from_machine` | `crates/rs_cam_core/src/feeds/explain_payload.rs:36` |
| struct | `FormulaBreakdown` | `crates/rs_cam_core/src/feeds/mod.rs:619` |
| const | `POWER_LADDER_AP_FLOOR_MM` | `crates/rs_cam_core/src/feeds/mod.rs:1154` |
| const | `POWER_LADDER_AE_FLOOR_MM` | `crates/rs_cam_core/src/feeds/mod.rs:1158` |
| const | `COLLET_EXPOSURE_MARGIN_MM` | `crates/rs_cam_core/src/feeds/predict.rs:79` |
| struct | `Predictions` | `crates/rs_cam_core/src/feeds/profile.rs:47` |
| struct | `ConstraintEnvelopes` | `crates/rs_cam_core/src/feeds/profile.rs:66` |
| fn | `edge_radius_floor` | `crates/rs_cam_core/src/feeds/provenance.rs:76` |
| fn | `from_chipload_source` | `crates/rs_cam_core/src/feeds/provenance.rs:95` |
| fn | `render_label` | `crates/rs_cam_core/src/feeds/quantities.rs:275` |
| const | `LEGACY_ESTIMATE_NOTE` | `crates/rs_cam_core/src/feeds/rationale.rs:56` |
| const | `MIN_CHIPLOAD_RANGE_FRACTION` | `crates/rs_cam_core/src/feeds/vendor_lut.rs:441` |
| fn | `detect_conflicting_rows` | `crates/rs_cam_core/src/feeds/vendor_lut.rs:551` |
| struct | `FinishSurfaceCacheStats` | `crates/rs_cam_core/src/finish_surface_cache.rs:263` |
| struct | `GeomCacheStats` | `crates/rs_cam_core/src/geom_cache.rs:166` |
| struct | `GrblImport` | `crates/rs_cam_core/src/machine_kinematics.rs:175` |
| struct | `MoveKinematics` | `crates/rs_cam_core/src/machine_kinematics.rs:488` |
| struct | `WindingReport` | `crates/rs_cam_core/src/mesh.rs:24` |
| struct | `ReachMapCacheStats` | `crates/rs_cam_core/src/reach_map_cache.rs:100` |
| fn | `scallop_toolpath_structured_annotated` | `crates/rs_cam_core/src/scallop.rs:1877` |
| struct | `ToolpathSemanticWriter` | `crates/rs_cam_core/src/semantic_trace.rs:934` |
| struct | `ToolpathSummary` | `crates/rs_cam_core/src/session/mod.rs:1001` |
| struct | `ToolSummary` | `crates/rs_cam_core/src/session/mod.rs:1026` |
| struct | `RebasedCuttingTimes` | `crates/rs_cam_core/src/simulation_cut.rs:694` |
| fn | `spiral_finish_toolpath_structured_annotated` | `crates/rs_cam_core/src/spiral_finish.rs:113` |
| struct | `CompactSpiral` | `crates/rs_cam_core/src/spiral_finish_compact.rs:260` |
| struct | `TierMapCacheStats` | `crates/rs_cam_core/src/tier_map_cache.rs:144` |
| fn | `first_span_of_kind` | `crates/rs_cam_core/src/tool_load/locality.rs:54` |
| fn | `resolve_rpm_bounds` | `crates/rs_cam_core/src/tool_load/optimize/bounds.rs:259` |
| fn | `marginal_safe` | `crates/rs_cam_core/src/tool_load/optimize/outcome.rs:498` |
| fn | `trade_off` | `crates/rs_cam_core/src/tool_load/optimize/outcome.rs:517` |
| fn | `first_marginal_safe_index` | `crates/rs_cam_core/src/tool_load/optimize/outcome.rs:649` |
| struct | `AxesPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:45` |
| struct | `FeedPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:68` |
| struct | `RetargetPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:79` |
| struct | `StagePolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:137` |
| struct | `FallbackPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:144` |
| struct | `PlungeStressWarning` | `crates/rs_cam_core/src/tool_load/plunge_stress.rs:44` |
| fn | `milling_criteria` | `crates/rs_cam_core/src/tool_load/verdict.rs:345` |
| struct | `RoutedLink` | `crates/rs_cam_core/src/unified_finish.rs:263` |
| struct | `GenerateAllScope` | `crates/rs_cam_viz/src/controller/generate_all.rs:20` |
| struct | `OptimizeStageRow` | `crates/rs_cam_viz/src/state/mod.rs:415` |
| struct | `SimulationRuntimeHotspot` | `crates/rs_cam_viz/src/state/simulation.rs:106` |
| struct | `ActiveCutSample` | `crates/rs_cam_viz/src/state/simulation.rs:123` |
| fn | `semantic_runtime_metrics` | `crates/rs_cam_viz/src/state/simulation.rs:1350` |
| fn | `current_cut_sample` | `crates/rs_cam_viz/src/state/simulation.rs:1385` |
| fn | `runtime_hotspots` | `crates/rs_cam_viz/src/state/simulation.rs:1413` |
| struct | `ToolpathMoveVisibility` | `crates/rs_cam_viz/src/state/viewport.rs:37` |
| struct | `ApplyReport` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:1330` |
