# Dead public surface (mechanical)

Definitions: 7270 pub items in production code. Word-count instrument over crates/*/{src,tests,benches,examples}; a name that occurs once in the whole workspace is its own definition and nothing else. Trait-impl methods and macro-generated uses are blind spots: VERIFY each row with rg before acting.

## Zero references anywhere (11)

| kind | name | file:line | vis |
|---|---|---|---|
| fn | `shared_flag` | `crates/rs_cam_core/src/budget/guard.rs:147` | pub |
| fn | `from_system_fraction` | `crates/rs_cam_core/src/budget/mod.rs:88` | pub |
| fn | `holder_diameter_mm` | `crates/rs_cam_core/src/machine/head.rs:334` | pub |
| fn | `ray_has_added_material` | `crates/rs_cam_core/src/stock/dexel.rs:285` | pub |
| fn | `derive_op_params` | `crates/rs_cam_derive/src/lib.rs:46` | pub |
| fn | `derive_param_enum` | `crates/rs_cam_derive/src/lib.rs:55` | pub |
| const | `COLLISION_POINT` | `crates/rs_cam_viz/src/render/colors.rs:44` | pub |
| fn | `resolved_text` | `crates/rs_cam_viz/src/state/shoe_plan.rs:204` | pub |
| fn | `detail_opt` | `crates/rs_cam_viz/src/ui/components/line.rs:45` | pub |
| const | `SPACE_0` | `crates/rs_cam_viz/src/ui/tokens.rs:36` | pub |
| fn | `accent_wash` | `crates/rs_cam_viz/src/ui/tokens.rs:757` | pub |

## `pub` but referenced only inside its own file (144) — visibility too wide, or dead behind a same-file caller

| kind | name | file:line |
|---|---|---|
| fn | `adaptive_3d_toolpath_structured_annotated_traced_with_cancel` | `crates/rs_cam_core/src/adaptive3d/mod.rs:618` |
| const | `TRACE_SAMPLE_STEP_FLOOR_MM` | `crates/rs_cam_core/src/budget/estimate.rs:327` |
| fn | `with_flag` | `crates/rs_cam_core/src/budget/guard.rs:113` |
| fn | `signed_distance` | `crates/rs_cam_core/src/compute/alignment_pins/walls.rs:132` |
| fn | `with_ramp_fold_lap_cap` | `crates/rs_cam_core/src/compute/catalog.rs:261` |
| const | `ZERO_DEPTH_EPSILON` | `crates/rs_cam_core/src/compute/config.rs:375` |
| const | `HELIX_ENTRY` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:182` |
| const | `PLUNGE_ENTRY` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:188` |
| const | `DOGBONES` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:194` |
| const | `LEAD_IN_OUT` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:200` |
| const | `LINK_MOVES` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:206` |
| const | `ARC_FIT` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:212` |
| const | `SEGMENT_MERGE` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:218` |
| const | `AIR_CUT_FILTER` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:224` |
| const | `FEED_OPTIMIZATION` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:230` |
| fn | `max_ceiling_z` | `crates/rs_cam_core/src/compute/obstacle.rs:1056` |
| type | `GroupObstacles` | `crates/rs_cam_core/src/compute/simulate.rs:170` |
| fn | `of_stock_change` | `crates/rs_cam_core/src/compute/source_stock.rs:101` |
| fn | `is_tip_size` | `crates/rs_cam_core/src/compute/tool_config.rs:842` |
| fn | `measured_labels` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:1413` |
| fn | `unmeasured_labels` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:1429` |
| struct | `OutsideRegionChord` | `crates/rs_cam_core/src/dressup/entry_audit.rs:142` |
| struct | `ReferenceEngagement` | `crates/rs_cam_core/src/dressup/mod.rs:1911` |
| const | `STRAIGHT_RUN_TOLERANCE_DEG` | `crates/rs_cam_core/src/dressup/tabs.rs:304` |
| const | `HARDNESS_EXACT_TOL` | `crates/rs_cam_core/src/feeds/extrapolation/hardness.rs:101` |
| struct | `FormulaBreakdown` | `crates/rs_cam_core/src/feeds/mod.rs:726` |
| const | `TIP_CAP_RULE_TEXT` | `crates/rs_cam_core/src/feeds/plunge.rs:37` |
| struct | `ConstraintEnvelopes` | `crates/rs_cam_core/src/feeds/profile.rs:56` |
| fn | `fraction_of_ceiling` | `crates/rs_cam_core/src/feeds/quantities.rs:265` |
| const | `PLUNGE_SLOPE_RULE_TEXT` | `crates/rs_cam_core/src/feeds/ramp.rs:57` |
| const | `DRILL_FORMULA_SOURCE` | `crates/rs_cam_core/src/feeds/support.rs:51` |
| const | `UNJUDGED_REASON` | `crates/rs_cam_core/src/feeds/support.rs:75` |
| const | `DRILL_FLAT` | `crates/rs_cam_core/src/feeds/support.rs:117` |
| const | `DRILL_BULL` | `crates/rs_cam_core/src/feeds/support.rs:123` |
| fn | `is_judged_wood` | `crates/rs_cam_core/src/feeds/support.rs:227` |
| fn | `calculator_formula_source` | `crates/rs_cam_core/src/feeds/support.rs:485` |
| fn | `formula_source_for_input` | `crates/rs_cam_core/src/feeds/support.rs:499` |
| struct | `CompactSpiral` | `crates/rs_cam_core/src/finish/spiral_finish_compact.rs:260` |
| struct | `RoutedLink` | `crates/rs_cam_core/src/finish/unified_finish.rs:309` |
| fn | `enforce_unmodeled_policy` | `crates/rs_cam_core/src/gcode/mod.rs:1077` |
| struct | `MoveKinematics` | `crates/rs_cam_core/src/machine/kinematics.rs:636` |
| const | `TOOL_CHANGE_MODE_IGNORE` | `crates/rs_cam_core/src/machine/mod.rs:333` |
| const | `CURTI_2021_SOURCE_ID` | `crates/rs_cam_core/src/material/force_line.rs:60` |
| const | `GOLI_2018_SOURCE_ID` | `crates/rs_cam_core/src/material/force_line.rs:63` |
| const | `CURTI_CHIP_RANGE_MM` | `crates/rs_cam_core/src/material/force_line.rs:78` |
| const | `CURTI_GRAIN_ANGLE_RANGE_DEG` | `crates/rs_cam_core/src/material/force_line.rs:83` |
| const | `GOLI_2018_MDF_CHIP_RANGE_MM` | `crates/rs_cam_core/src/material/force_line.rs:86` |
| const | `GOLI_2018_MDF_KS_N_PER_MM2` | `crates/rs_cam_core/src/material/force_line.rs:90` |
| const | `GOLI_2018_MDF_INT_N_PER_MM` | `crates/rs_cam_core/src/material/force_line.rs:94` |
| const | `GOLI_2018_MDF_DENSITY_KG_M3` | `crates/rs_cam_core/src/material/force_line.rs:98` |
| const | `FPL_12PCT_MOISTURE_FACTOR` | `crates/rs_cam_core/src/material/force_line.rs:102` |
| const | `FPL_TABLE_5_5A_SOURCE_ID` | `crates/rs_cam_core/src/material/force_line.rs:105` |
| const | `FPL_CH4_SG_CONVERSION_SOURCE_ID` | `crates/rs_cam_core/src/material/force_line.rs:108` |
| const | `FPL_TABLE_5_3A_SOURCE_ID` | `crates/rs_cam_core/src/material/force_line.rs:111` |
| const | `FPL_EQ_4_11_COEFF` | `crates/rs_cam_core/src/material/force_line.rs:114` |
| const | `FPL_FIBRE_SATURATION_MC_PCT` | `crates/rs_cam_core/src/material/force_line.rs:119` |
| const | `FPL_TARGET_MC_PCT` | `crates/rs_cam_core/src/material/force_line.rs:123` |
| fn | `kc_eq` | `crates/rs_cam_core/src/material/force_line.rs:598` |
| const | `DRILL_PLUNGE_CEILING_WOOD_PER_MM` | `crates/rs_cam_core/src/material/mod.rs:652` |
| const | `DRILL_PLUNGE_CEILING_PLYWOOD_PER_MM` | `crates/rs_cam_core/src/material/mod.rs:656` |
| const | `DRILL_PLUNGE_CEILING_SHEET_PER_MM` | `crates/rs_cam_core/src/material/mod.rs:661` |
| const | `DRILL_PLUNGE_FLOOR_WOOD_PER_MM` | `crates/rs_cam_core/src/material/mod.rs:665` |
| const | `DRILL_PLUNGE_FLOOR_BOARD_PER_MM` | `crates/rs_cam_core/src/material/mod.rs:669` |
| const | `FPL_CH5_SOURCE_ID` | `crates/rs_cam_core/src/material/wood_species_library.rs:25` |
| struct | `WindingReport` | `crates/rs_cam_core/src/mesh.rs:24` |
| const | `FLAT_FACET_NORMAL_Z` | `crates/rs_cam_core/src/ops/waterline.rs:112` |
| const | `FLAT_FACE_SNAP_MM` | `crates/rs_cam_core/src/ops/waterline.rs:114` |
| const | `STOCK_BOTTOM_FLOOR_EPS_MM` | `crates/rs_cam_core/src/ops/waterline.rs:118` |
| fn | `flat_facet_z_values` | `crates/rs_cam_core/src/ops/waterline.rs:140` |
| struct | `WaterlineLevel` | `crates/rs_cam_core/src/ops/waterline.rs:336` |
| fn | `as_curves` | `crates/rs_cam_core/src/selection/view.rs:93` |
| struct | `ToolSummary` | `crates/rs_cam_core/src/session/mod.rs:1012` |
| const | `TIER_ARC_TOLERANCE_PER_CUSP_HEIGHT` | `crates/rs_cam_core/src/session/multitool.rs:917` |
| fn | `resolved_shoe_for_toolpath` | `crates/rs_cam_core/src/session/shoe.rs:234` |
| fn | `model_ids_label` | `crates/rs_cam_core/src/session/stock_change_report.rs:89` |
| const | `DEFAULT_WINDOW_SIZE` | `crates/rs_cam_core/src/settings/mod.rs:203` |
| const | `DEFAULT_PLAYBACK_SPEED` | `crates/rs_cam_core/src/settings/mod.rs:267` |
| const | `STOCK_OPACITY` | `crates/rs_cam_core/src/settings/mod.rs:364` |
| fn | `merged_text` | `crates/rs_cam_core/src/settings/mod.rs:833` |
| fn | `config_dir_from` | `crates/rs_cam_core/src/settings/paths.rs:63` |
| fn | `cache_dir_from` | `crates/rs_cam_core/src/settings/paths.rs:83` |
| fn | `installed_paths` | `crates/rs_cam_core/src/settings/paths.rs:191` |
| const | `POPULATION_EXCLUDED` | `crates/rs_cam_core/src/stock/cut_as.rs:245` |
| fn | `ray_top_material` | `crates/rs_cam_core/src/stock/dexel.rs:279` |
| fn | `slack_for` | `crates/rs_cam_core/src/tool_load/boundary.rs:88` |
| const | `DEFAULT_BIN_COUNT` | `crates/rs_cam_core/src/tool_load/distribution.rs:72` |
| const | `RANGE_PERCENTILE_LOW` | `crates/rs_cam_core/src/tool_load/distribution.rs:76` |
| const | `RANGE_PERCENTILE_HIGH` | `crates/rs_cam_core/src/tool_load/distribution.rs:80` |
| fn | `band_range` | `crates/rs_cam_core/src/tool_load/mod.rs:201` |
| struct | `OwnMaterialVerdict` | `crates/rs_cam_core/src/tool_load/mod.rs:596` |
| struct | `PlungeStressWarning` | `crates/rs_cam_core/src/tool_load/plunge_stress.rs:55` |
| fn | `from_status` | `crates/rs_cam_core/src/tool_load/verdict.rs:495` |
| struct | `ToolLoadReportSummary` | `crates/rs_cam_core/src/tool_load/verdict.rs:523` |
| const | `GANTRY_PUSH_UNMODELED_CLAUSE` | `crates/rs_cam_core/src/tool_load/verdict.rs:722` |
| const | `GANTRY_PUSH_NOT_APPLICABLE_CLAUSE` | `crates/rs_cam_core/src/tool_load/verdict.rs:727` |
| struct | `DiameterRangeParam` | `crates/rs_cam_mcp/src/server.rs:945` |
| struct | `ZRangeParam` | `crates/rs_cam_mcp/src/server.rs:963` |
| struct | `AlignmentPinParam` | `crates/rs_cam_mcp/src/server.rs:1633` |
| struct | `Proxy` | `crates/rs_cam_mcp_proxy/src/proxy.rs:144` |
| struct | `GenerateAllScope` | `crates/rs_cam_viz/src/controller/generate_all.rs:52` |
| fn | `toolpath_setup_frame` | `crates/rs_cam_viz/src/interaction/picking.rs:302` |
| fn | `feeds_caution_report` | `crates/rs_cam_viz/src/io/export.rs:60` |
| fn | `export_setup_gcode_from_session_reporting` | `crates/rs_cam_viz/src/io/export.rs:984` |
| fn | `shelf_draw_radius` | `crates/rs_cam_viz/src/render/sim_render.rs:595` |
| const | `PIN_MARK_LIFT_MM` | `crates/rs_cam_viz/src/state/pin_view.rs:21` |
| struct | `ToolpathMoveVisibility` | `crates/rs_cam_viz/src/state/viewport.rs:37` |
| fn | `is_overflow_bin` | `crates/rs_cam_viz/src/ui/components/histogram.rs:483` |
| const | `LINE_WIDTH` | `crates/rs_cam_viz/src/ui/components/sparkline.rs:80` |
| const | `MEDIAN_WIDTH` | `crates/rs_cam_viz/src/ui/components/sparkline.rs:83` |
| const | `MARKER_SPACING` | `crates/rs_cam_viz/src/ui/components/sparkline.rs:93` |
| const | `FLIP_SIZE` | `crates/rs_cam_viz/src/ui/components/sparkline.rs:96` |
| fn | `move_of` | `crates/rs_cam_viz/src/ui/components/sparkline.rs:147` |
| struct | `LineRange` | `crates/rs_cam_viz/src/ui/components/sparkline.rs:283` |
| const | `COUNT_GLYPH` | `crates/rs_cam_viz/src/ui/components/tab_strip.rs:28` |
| const | `LEGEND_MAX_WIDTH` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:55` |
| const | `LEGEND_MIN_WIDTH_BESIDE` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:59` |
| const | `DETAIL_MAX_WIDTH` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:62` |
| const | `GIZMO_CLEARANCE` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:71` |
| fn | `dressup_rgb` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:1257` |
| fn | `rapid_rgb` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:1266` |
| const | `SIMULATION_STALE_NOTE` | `crates/rs_cam_viz/src/ui/overlays/live.rs:117` |
| const | `COLLISION_STALE_NOTE` | `crates/rs_cam_viz/src/ui/overlays/live.rs:120` |
| fn | `toolpath_number` | `crates/rs_cam_viz/src/ui/overlays/live.rs:131` |
| fn | `preferred_mode` | `crates/rs_cam_viz/src/ui/overlays/panel.rs:562` |
| const | `REST_CONFIRM_BODY` | `crates/rs_cam_viz/src/ui/overlays/panel.rs:666` |
| struct | `ApplyReport` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:1553` |
| fn | `is_advanced` | `crates/rs_cam_viz/src/ui/preferences.rs:115` |
| fn | `overlay_choice` | `crates/rs_cam_viz/src/ui/preferences.rs:463` |
| fn | `add_part` | `crates/rs_cam_viz/src/ui/properties/head.rs:119` |
| fn | `make_shelf` | `crates/rs_cam_viz/src/ui/properties/head.rs:137` |
| fn | `along_on_axis` | `crates/rs_cam_viz/src/ui/properties/pins/edit.rs:164` |
| struct | `PlanViewOut` | `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:48` |
| fn | `to_mm` | `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:97` |
| fn | `scanline_spans` | `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:428` |
| struct | `StockPanelOut` | `crates/rs_cam_viz/src/ui/properties/stock.rs:222` |
| const | `CUT_AS_HOVER` | `crates/rs_cam_viz/src/ui/properties/stock_changes.rs:145` |
| fn | `cycle_time_evidence` | `crates/rs_cam_viz/src/ui/readiness.rs:483` |
| fn | `apply_to_style` | `crates/rs_cam_viz/src/ui/tokens.rs:431` |
| fn | `worst_edge` | `crates/rs_cam_viz/src/ui/toolpath_panel.rs:1342` |
| const | `FULL_DOCK_MIN_VIEWPORT_WIDTH` | `crates/rs_cam_viz/src/ui/viewport_overlay.rs:63` |
| const | `LABELLED_CATALOGUE_MIN_WIDTH` | `crates/rs_cam_viz/src/ui/viewport_overlay.rs:68` |
| const | `POPOVER_MAX_WIDTH` | `crates/rs_cam_viz/src/ui/viewport_overlay.rs:71` |
| fn | `section_button_text` | `crates/rs_cam_viz/src/ui/viewport_overlay.rs:233` |
| fn | `target_text` | `crates/rs_cam_viz/src/ui/viewport_overlay.rs:252` |
