# Dead public surface (mechanical)

Definitions: 4984 pub items in production code. Word-count instrument over crates/*/{src,tests,benches,examples}; a name that occurs once in the whole workspace is its own definition and nothing else. Trait-impl methods and macro-generated uses are blind spots: VERIFY each row with rg before acting.

## Zero references anywhere (4)

| kind | name | file:line | vis |
|---|---|---|---|
| fn | `append_toolpath` | `crates/rs_cam_core/src/trace/semantic_trace.rs:949` | pub |
| const | `COLLISION_POINT` | `crates/rs_cam_viz/src/render/colors.rs:41` | pub |
| fn | `row_hover_tint` | `crates/rs_cam_viz/src/ui/components/kv_row.rs:250` | pub |
| const | `SPACE_0` | `crates/rs_cam_viz/src/ui/tokens.rs:36` | pub |

## `pub` but referenced only inside its own file (34) — visibility too wide, or dead behind a same-file caller

| kind | name | file:line |
|---|---|---|
| struct | `ParamHint` | `crates/rs_cam_core/src/compute/catalog.rs:1214` |
| struct | `OperationParamSchema` | `crates/rs_cam_core/src/compute/catalog.rs:1229` |
| struct | `NewDefaultCtx` | `crates/rs_cam_core/src/compute/catalog.rs:2679` |
| fn | `floating_fraction` | `crates/rs_cam_core/src/compute/config.rs:1379` |
| struct | `OutsideRegionChord` | `crates/rs_cam_core/src/dressup/entry_audit.rs:142` |
| struct | `ReferenceEngagement` | `crates/rs_cam_core/src/dressup/mod.rs:2709` |
| struct | `FormulaBreakdown` | `crates/rs_cam_core/src/feeds/mod.rs:619` |
| struct | `Predictions` | `crates/rs_cam_core/src/feeds/profile.rs:47` |
| struct | `ConstraintEnvelopes` | `crates/rs_cam_core/src/feeds/profile.rs:66` |
| fn | `scallop_toolpath_structured_annotated` | `crates/rs_cam_core/src/finish/scallop.rs:1880` |
| fn | `spiral_finish_toolpath_structured_annotated` | `crates/rs_cam_core/src/finish/spiral_finish.rs:113` |
| struct | `CompactSpiral` | `crates/rs_cam_core/src/finish/spiral_finish_compact.rs:260` |
| struct | `RoutedLink` | `crates/rs_cam_core/src/finish/unified_finish.rs:265` |
| struct | `GrblImport` | `crates/rs_cam_core/src/machine/kinematics.rs:175` |
| struct | `MoveKinematics` | `crates/rs_cam_core/src/machine/kinematics.rs:488` |
| struct | `FinishSurfaceCacheStats` | `crates/rs_cam_core/src/maps/finish_surface_cache.rs:263` |
| struct | `GeomCacheStats` | `crates/rs_cam_core/src/maps/geom_cache.rs:166` |
| struct | `ReachMapCacheStats` | `crates/rs_cam_core/src/maps/reach_map_cache.rs:100` |
| struct | `TierMapCacheStats` | `crates/rs_cam_core/src/maps/tier_map_cache.rs:144` |
| struct | `WindingReport` | `crates/rs_cam_core/src/mesh.rs:24` |
| struct | `ToolpathSummary` | `crates/rs_cam_core/src/session/mod.rs:1001` |
| struct | `ToolSummary` | `crates/rs_cam_core/src/session/mod.rs:1026` |
| struct | `RebasedCuttingTimes` | `crates/rs_cam_core/src/stock/simulation_cut.rs:694` |
| struct | `PlungeStressWarning` | `crates/rs_cam_core/src/tool_load/plunge_stress.rs:44` |
| struct | `ToolpathSemanticWriter` | `crates/rs_cam_core/src/trace/semantic_trace.rs:936` |
| struct | `GenerateAllScope` | `crates/rs_cam_viz/src/controller/generate_all.rs:20` |
| struct | `OptimizeStageRow` | `crates/rs_cam_viz/src/state/mod.rs:415` |
| struct | `SimulationRuntimeHotspot` | `crates/rs_cam_viz/src/state/simulation.rs:106` |
| struct | `ActiveCutSample` | `crates/rs_cam_viz/src/state/simulation.rs:123` |
| fn | `semantic_runtime_metrics` | `crates/rs_cam_viz/src/state/simulation.rs:1350` |
| fn | `current_cut_sample` | `crates/rs_cam_viz/src/state/simulation.rs:1385` |
| fn | `runtime_hotspots` | `crates/rs_cam_viz/src/state/simulation.rs:1413` |
| struct | `ToolpathMoveVisibility` | `crates/rs_cam_viz/src/state/viewport.rs:37` |
| struct | `ApplyReport` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:1330` |
