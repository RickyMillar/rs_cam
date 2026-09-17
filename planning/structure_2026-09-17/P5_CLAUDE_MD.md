# P5 — folder-level `CLAUDE.md` files

Date: 2026-09-17. Owner: the P5 documentation agent.

## 1. The request

The operator asked for one instruction file per module, so that no single
file carries every rule. Claude Code loads a nested `CLAUDE.md` only when the
work touches that directory. A folder file therefore costs nothing until a
developer edits that folder.

The shape:

- The root `CLAUDE.md` is the workspace index and the quality gates.
- A crate `CLAUDE.md` holds the crate-wide contracts and a one-line index per
  folder. Each index row names the folder file where one exists.
- A folder `CLAUDE.md` holds only what a developer who edits that folder must
  know. Maximum 40 lines.

## 2. The admission rule

A folder gets a file when at least one of these is true:

- The folder owns an invariant that a developer can break.
- The folder has a sentry that a developer must run.
- The folder holds more than about four files that need a one-line map.

A folder that meets none of them keeps its row in the crate table and gets no
file.

## 3. No sentence appears twice

The parent indexes. The child holds the rule. Every sentence that moves is
deleted from the parent in the same commit. Section 6 lists each move by
source line, so the check is mechanical.

## 4. The line budget

Four folders cannot carry a one-line-per-file map and the invariants and the
sentries inside 40 lines: `compute/`, `feeds/`, `finish/`, `tool_load/`, and
`ui/` in viz. In those folders the map groups a family on one line, and a
child directory (`compute/execute/`, `finish/pencil/`, `tool_load/optimize/`)
is one line, not one line per child file.

The P4 split agents still move code today. A file map therefore names the
module and says "and its `<name>/` children". It does not enumerate a split
artifact that may be renamed tomorrow.

## 5. The folder table — `crates/rs_cam_core/src/`

`Sentries` names the test binaries under `crates/rs_cam_core/tests/`. Each was
ranked by counting that folder's public symbol names across the 337 test
binaries; the raw ranking is reproducible with the harvest in section 9.

| Folder | File? | Sentences that move in | Sentries |
|---|---|---|---|
| `adaptive/` | yes | none; map + sentries only | `adaptive_property_harness`, `contour_spiral_gcode_validity_phase0` |
| `adaptive3d/` | yes | D-adaptive3d (all three `ClearingStrategy3d` variants live; `clear_z_level` is not dead; a vertical descent is classified by geometry, not by intent tag) | `adaptive3d_boundary_clear_parity`, `adaptive3d_keep_down_link_f038b`, `adaptive3d_entry_coalescing_f038`, `agent_search_coverage`, `adaptive3d_subtool_channel_gouge` |
| `compute/` | yes | core:74-76 (the phantom prior-stock rule for the first pending rest op) | `retract_intent_move_type_census_w6`, `capability_link_moves_safety`, `sim_prefix_memo_s5`, `set_param_refuses_absent_field_n5`, `generated_empty_refusal_g_entryempty` |
| `dexel_stock/` | yes | D-stock part 1 (2D operations cut at negative Z; `StockConfig.origin_z` is negative; the stock top is at Z=0), D-stock part 3 (the sim cell must be much smaller than the tool tip radius) | `dexel_stock_z_frame_f024`, `playback_band_dispatch_s6`, `swept_stamping_s1`, `band_stamping_determinism_s3`, `sub_cell_stamping_fa` |
| `diagnostics/` | yes | core:53-55 (`None` versus `Some(0.0)` for a measured finding) | `depth_beyond_stock_core_g_depthstockcore`, `chipload_abstention_cannot_supersede_g_chipgate`, `drill_evidence_wording_d3`, `region_cap_honesty_f3`, `gate_population_vacuity_xvac` |
| `dressup/` | yes | D-dressup (the retract-strategy dial is dead, G-RETRACTDIAL; linking goes through the shared `relink_fragments` kernel; segment-merge conditioning is default-on for roughing) | `capability_link_moves_safety`, `constrained_max_modulation_f039`, `entry_moves_stock_aware_g_rampterrain`, `lead_in_out_feed_rates_f040`, `plunge_guard_p3` |
| `export/` | yes | D-gcode part 2 (export zeroes at the stock top; a flipped setup zeroes to the presented top) | `composite_render_convention`, `gcode_validator_baseline`, `param_sweep` |
| `feeds/` | yes | core:84-87 (Suggest is the validated path; a matched vendor band caps the rubbing floor), D-feeds (`ChiploadBounds` mirrors the gate's DOC derating, canonical scale in `feeds::geometry`; the LUT lookup is hardness-agnostic; trust the simulated chipload on an adaptive rough) | `lut_resolver_census_a6`, `wanaka_suggest_integration`, `lookup_parity`, `rubbing_floor_never_exceeds_band`, `pill_writes_clamped_value_g_pillclamp` |
| `finish/` | yes | D-finish (pre-2026-08-04 strategy verdicts are superseded; the pencil NMS detector stands; `crate::surface::flow_accum` is kept on purpose; the iso-scallop `iso_field` dial; never gate on an aggregate without rendering the surface) | `finish_resolution_policy_pr3`, `conformal_spiral_synthetic_f2`, `classification_strategy_m3`, `finish_planner_wanaka_decompose`, `scallop_iso_field_config` |
| `gcode/` | yes | D-gcode part 1 (the datum: StockTop maps to Z0) | `gcode_phase0_capture`, `post_format_round_trip_p1`, `export_honors_coolant_p0d1`, `export_datum_setup_frame`, `gcode_validator_baseline` |
| `geometry/` | yes | core:97-98 first half (setup-local versus world-frame boundaries), core:105-106 (`ToolContainment::Inside` falls back to unclipped geometry; inspect `boundary_clip_dropped`) | `boundary_clip_escape_f1`, `monotone_cell_decomposition_c2`, `skipped_boundary_offset_f8`, `nn_order_scaling_g5_g6`, `thin_organic_island_widths` |
| `io/` | yes | none; map + sentries only | `step_import`, `step_project_load`, `model_units_survive_reload_g_unitsreload`, `drill_picks_resolve_to_targets_g_drillpickstale`, `model_path_round_trip_g_modelrelink` |
| `machine/` | yes | none; map + sentries only | `kinematics_per_axis_rate_p1`, `kinematic_utilization_p2`, `kinematic_surfacing_p4`, `a_downward_traverse_rounds_down_g_rpmdown`, `power_ceiling_parity_f2` |
| `maps/` | yes | core:80-83 (the reach map is a top-down, rim-eroded upper estimate; red at or below the discretisation floor is unresolved arithmetic; `stock_to_leave` is an offset, never a reach tolerance) | `reach_map_p5`, `reach_map_residual_p5_1`, `tier_map_walk_t1`, `tier_map_slope_t2`, `tier_islands_i1` |
| `material/` | **no** | 2 files, no invariant of its own | — |
| `metrology/` | yes | none; map + sentries only | `bikeseat_gate_d1`, `zone_coherence_census`, `union_coverage_m1`, `wanaka_curvature_anisotropy`, `spacing_prize_split_f1` |
| `ops/` | yes | core:97-99 second half (drill removal uses the supplied `StockCutDirection`), core:100-104 (a pinned Bottom Z is honoured only by `Adaptive3d`, `UnifiedFinish` and `Waterline`; explicit-depth operations diagnose a cut below the stock bottom, others abstain), core:88-90 last clause (the drill metrics live in `ops/drill_metrics.rs`) | `drill_op_step3`, `drill_flip_removal_g_drillflip`, `depth_beyond_stock_core_g_depthstockcore`, `project_curve_depth_sign`, `drill_evidence_wording_d3` |
| `session/` | yes | core:47-48 (the mutation door returns `Effects` with the authoritative stale set), core:51-52 (an edit invalidates the cached chain), core:59-61 (read `simulation_triage` before a raw issue count), D-session (`setups_mut` is `#[cfg(test)]`) | `command_registry_completeness`, `mutation_paths_invalidate_alike_p0`, `adopt_result_rejects_stale_completion`, `replace_toolpath_config_gates_on_the_signature`, `stale_set_has_one_answer_wp28` |
| `stock/` | yes | core:56-57 (a gate with no population proved nothing), core:62-69 (the air-cut denominator, `average_engagement` is comparative), core:71-72 (`NotMeasurable` abstains; collision detection stays on), D-stock part 2 (`claims_reference` must be `machined_stock` in a cascade) and part 4 (re-simulating does not stale a toolpath; regenerate the rest ops after) | `measurability_abstention_r8`, `air_cut_one_time_base_g_airdenom`, `narration_denominator_and_hints_d7`, `gate_population_vacuity_xvac`, `engagement_denominator_m3` |
| `surface/` | yes | none; map + sentries only | `reach_policy_pr4`, `grid_z_uncovered_contract_c2`, `rest_routing_probe_e9`, `catchment_basin_census_w0`, `drop_cutter_off_mesh` |
| `tool/` | yes | D-maps last clause, **reassigned** (the cusp radius is what the tool forms, the valley radius is what it fits into; the two diverge on a bull nose) | `tool_scale_semantics_pr2`, `bull_nose_cusp_radius_g_bullcusp`, `tapered_cusp_radius_sentry`, `tool_geometry_hygiene`, `tapered_ball_relief_profile_probe` |
| `tool_load/` | yes | core:70-72 first half (the chipload quantity is advance per tooth), core:88-93 (the drill gates model the R-plane-rooted schedule, read cutting geometry not fed distance, and use the envelope diameter on a tapered drill), D-tool_load (one predicate `locality::is_steady_state_for_gate`; a Kc or factor change needs the slow `--test` sims) | `gate_population_vacuity_xvac`, `chipload_boundary_g_chip_ulp`, `chipload_abstention_cannot_supersede_g_chipgate`, `predicted_feed_gates_f035`, `drill_evidence_wording_d3` |
| `trace/` | yes | D-trace (a trace with no provenance block is stale; the `narrate` Z ladder is nominal, not achieved) | `remap_interval_index_c9`, `transform_provenance_fingerprints`, `exporter_span_classifier_x1`, `narrate_regions_closed_c8`, `narration_cost_probe_h26` |
| `util/` | **no** | 3 trivial files | — |

Files: 22 of 24 folders.

## 6. The folder table — `crates/rs_cam_viz/src/`

| Directory | File? | Sentences that move in | Sentries |
|---|---|---|---|
| `app/` | yes | viz:47-62 (the embedded MCP server, the live-work order, the setter/regenerate rule, the export refusals, `generation_status`, the screenshot rule), D-viz-app (`.mcp.json` runs `cargo run --release`, so pre-build the release binary) | `mcp_authoring_surface`, `mcp_core_arm_describes_every_row`, `mcp_escape_hatches`, `mcp_wire_surface_pin`, `export_parity_core_vs_gui_p0` |
| `compute/` | yes | viz:16-18 second half (a worker result must drop an affected result) | `generate_all_fixpoint_parity`, `export_parity_core_vs_gui_p0`, `optimize_runs_on_the_job_lane_wp14b` |
| `controller/` | yes | viz:16-18 first half (the controller → worker → result-acceptance path), D-viz-controller (the view mirrors `Effects`; a toolpath edit clears the viewport simulation; `OptimizeToolpath` is a `Job` over a cloned session) | `apply_contract_a3`, `effects_are_stamped_wp19`, `production_writes_go_through_apply_wp15a`, `generate_all_fixpoint_parity`, `feeds_apply_drops_result_n13` |
| `interaction/` | **no** | 2 files | — |
| `io/` | **no** | 4 files; the field-audit rule stays crate-wide | — |
| `render/` | yes | none; map + sentries only | `render_pipelines_headless_g_pipesmoke`, `reach_overlay_p5`, `viewport_draws_selected_only_wp27` |
| `state/` | yes | viz:28-31 (metric-capture staleness is derived from the accepted run's capture revision) | `freshness_surfaces_g_freshrender`, `rest_badge_one_predicate_g_restbadge`, `effects_are_stamped_wp19`, `overlays_registry`, `load_requests_only_25d_regen_g_loadregen` |
| `ui/` | yes | viz:22-27 (one visible full-run primary; `simulation_request_is_buildable`), viz:32-43 minus the overlay row (the selected toolpath is the default draw set; the declutter rule), D-viz-ui (WP27; the egui 0.34.3 / `egui_plot` 0.35 trap; the design tokens held by the ui-premium plan) | `viewport_draws_selected_only_wp27`, `the_simulation_page_is_summary_first_dc6`, `ui_string_hygiene`, `panels_read_the_token_module_up1`, `workspace_menu_complete_g_wsmenu` |
| `ui/components/` | yes | none; map + the display contracts it already owns | `component_contracts_up2`, `chrome_reads_the_kit_up3`, `panels_read_the_token_module_up1`, `the_toolpath_card_is_five_elements_dc1` |
| `ui/feeds/` | yes | D-viz-controller last clause, **reassigned** (the Feeds tab never auto-locks a numeric field; it uses explicit Suggest buttons) | `the_feeds_modal_holds_one_scope_dc5a`, `the_feeds_window_fits_the_screen_g_feedsfit`, `the_recommendation_explains_each_row_g_whyrow`, `the_nomogram_readout_abstains_g_hoverbound`, `the_speeds_apply_holds_the_cut_g_speedsonly` |
| `ui/overlays/` | yes | viz:38-40 (one registry for panel, MCP and registration; an unavailable overlay is refused with a reason) | `overlays_registry`, `reach_overlay_p5` |
| `ui/properties/` | yes | none; map + sentries only | `bottom_z_pin_note_g_bottompin`, `boundary_controls_always_visible_g_boundaryinherit`, `the_inspector_nests_once_dc5`, `inspector_width_is_tab_independent_up4`, `depth_beyond_stock_cautions_g_depthstock` |

Files: 10 of 12 directories.

## 7. Reassignments

The task assigned two rules to a folder that does not define the symbol. The
rule follows the definition, so that the child that holds the rule is the
child a developer opens.

| Rule | Assigned to | Written into | Why |
|---|---|---|---|
| Cusp radius versus valley radius | `maps/` | `tool/` | `cusp_radius`, `cusp_radius_mm` and `valley_radius_mm` are defined in `tool/mod.rs`. |
| The Feeds tab never auto-locks a field | viz `controller/` | viz `ui/feeds/` | The Feeds tab and its Suggest buttons live in `ui/feeds/`. |

Two further splits, recorded so the no-duplicate check stays mechanical:

- The drill rules split. The gate rules go to `tool_load/`, because
  `tool_load/drill_gates.rs` defines them. The operation rules
  (`StockCutDirection`, the location of `drill_summaries`) go to `ops/`,
  because `ops/drill_op.rs` and `ops/drill_metrics.rs` define them.
- The stock rules split. `dexel_stock/` takes the Z-frame and the sim-cell
  rules. `stock/` takes triage, measurability, gate population and the
  cascade reference, because `stock/sim_triage.rs` and
  `stock/sim_measurability.rs` define them.

## 8. Dropped rules

None. Every symbol the task named still exists:

`ChiploadBounds` (`feeds/geometry.rs`), `is_steady_state_for_gate`
(`tool_load/locality.rs`), `honors_pinned_bottom_z` (`compute/catalog.rs`),
`ToolContainment` and `boundary_clip_dropped` (`geometry/boundary.rs`,
`compute/config.rs`), `relink_fragments` (`finish/surface_link.rs`),
`flow_accum` (`surface/flow_accum.rs`), `iso_field`
(`compute/operation_configs.rs`), `StockCutDirection`
(`dexel_stock/cut_direction.rs`), `NotMeasurable`
(`stock/sim_measurability.rs`), `claims_reference` and `machined_stock`
(`compute/config.rs`, `compute/operation_configs.rs`), `clear_z_level`
(`adaptive3d/clearing.rs`), `ClearingStrategy3d` (`adaptive3d/mod.rs`),
`setups_mut` (`session/mod.rs`, `#[cfg(test)]`), `origin_z`
(`compute/stock_config.rs`), `SPACE_0`, `INK_00`, `LANE_SCALE`
(`ui/tokens.rs`), `COLLISION_POINT` (`render/colors.rs`),
`draw_trace_badge` (`ui/sim_debug.rs`), `row_hover_tint`
(`ui/components/kv_row.rs`), `OptimizeToolpath` (`compute/worker.rs`).

The `retract_strategy` field still exists in `compute/config.rs`. The rule is
that the dial is dead, so the rule stands as a warning, not as a drop.

## 9. Heavy-gated sentries

Twelve core test binaries carry `required-features = ["heavy-tests"]`. A
command that names one of them must add the feature flag:

`cargo test -p rs_cam_core --features heavy-tests -q --test <name>`

The twelve: `feed_modulation_cycle_time_f036c`, `scallop_isofield_gouge_m4`,
`scallop_candidates_m4`, `machine_kinematics_cycle_time_f034`,
`offset_growth_m5`, `checkpoint_b_resolution_ab`,
`scallop_oracle_validation_m4`, `air_cut_family_calibration_w5bf4`,
`adaptive3d_planner_stock_xy_f027`, `offset_candidates_m5`,
`adaptive3d_interior_cell_parity_f029`, `strategy_advisor_smoke`.

Section 5 avoids all twelve, so every sentry command in a folder file runs
without the feature flag. This also respects the operator ruling of
2026-09-11: no large test gates.

The sentry ranking harvests `pub fn`, `pub struct`, `pub enum` and
`pub trait` names of nine characters or more from each folder, then counts
each name across `crates/rs_cam_core/tests/*.rs` and keeps the highest eight.
Entries that are not test binaries (`tests/common/shim.rs`,
`tests/fixtures/cells.toml`) are discarded by hand.

## 10. The parents after the trim

| File | Before | Target |
|---|---:|---:|
| `crates/rs_cam_core/CLAUDE.md` | 121 | ≤ 60 |
| `crates/rs_cam_viz/CLAUDE.md` | 68 | ≤ 40 |
| root `CLAUDE.md` | 74 | 74 + 1 sentence |

The core parent keeps the spine paragraph, the folder table with a
`→ <folder>/CLAUDE.md` pointer on each row that has one, the crate-wide
contracts, and "Tests and evidence" including the purge and tag note. The viz
parent keeps its intro, a directory table with the same pointers, the
crate-wide GUI contracts and the test line.

`crates/rs_cam_cli/CLAUDE.md` and `crates/rs_cam_mcp/CLAUDE.md` do not
change. Neither crate has a folder that meets the admission rule.

`planning/AGENT_CODEMAP.md` and `.claude/skills/dev/SKILL.md` each get one
pointer line. Neither duplicates a folder file's content.

## 11. Commits

1. This plan.
2. The 32 folder files.
3. The two trimmed parents, the root sentence, the codemap line and the skill
   line.

Every commit uses an explicit pathspec. A bare `git commit` in this shared
checkout sweeps a peer's staged work.
