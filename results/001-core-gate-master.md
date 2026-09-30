---
id: 001
state: done
commit: 5b34e710
started: 2026-09-30T19:14:15Z
finished: 2026-09-30T19:46:37Z
---
## Summary

13 failing targets, 0 build errors. Lib green. Wall 32 min 22 s on 24
cores (build ~3 min from the shared cache; the rest is the test run).

Failing targets:
- chipload_formula_calibration
- crease_own_region_pr6b
- cut_direction_matches_transform_g_lateralsign
- feed_explanation_snapshot_b3
- heatmap_two_arc_divergence_a1
- machine_kinematics_cycle_time_f034
- scallop_trace_survives_relink_g_linktrace
- setters_are_crate_private_wp15b
- setters_have_rows_wp15a
- simulation_issue_channel_m1
- tapered_width_model_parity_c3
- the_test_doors_are_gated_fld0405
- vendor_sidebyside_chipload
- chipload_formula_calibration
- crease_own_region_pr6b
- cut_direction_matches_transform_g_lateralsign
- feed_explanation_snapshot_b3
- heatmap_two_arc_divergence_a1
- machine_kinematics_cycle_time_f034
- scallop_trace_survives_relink_g_linktrace
- setters_are_crate_private_wp15b
- setters_have_rows_wp15a
- simulation_issue_channel_m1
- tapered_width_model_parity_c3
- the_test_doors_are_gated_fld0405
- vendor_sidebyside_chipload

Log on the runner's PC: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam--claude-worktrees-bridge-cse-01X9bPqEnCfUkrbNprBjUs3d/84f1aa02-8de5-5da0-aff8-1fa9722b654e/scratchpad/core_gate_5b34e710.log

- lib: `test result: ok. 2661 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 47.35s`
- `test result: ok` lines: 405; `test result: FAILED` lines: 13
- failing targets: 13
- wall time: 32 min 22 s (cargo rc=101)
- build errors: 0

## Detail

### chipload_formula_calibration
- failing tests: the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum
- first panic: `thread 'the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum' (503535) panicked at crates/rs_cam_core/tests/chipload_formula_calibration.rs:289:5:`
  `the exhibit is only worth keeping if the overshoot is large; got 1.42x`

### crease_own_region_pr6b
- failing tests: production_unified_finish_output_is_byte_identical
- first panic: `thread 'production_unified_finish_output_is_byte_identical' (508150) panicked at crates/rs_cam_core/tests/crease_own_region_pr6b.rs:592:5:`
  `H2.4 must not move a single emitted move -- taper: got (952, 0xfb81b05411cf5b98), pinned (951, 0x4b4666eba3afa7ab); ball: got (660, 0x3c0937d6984eb555), pinned (659, 0xb506292bc14e7e38)`

### cut_direction_matches_transform_g_lateralsign
- failing tests: no_session_file_open_codes_the_group_stock_rule
- first panic: `thread 'no_session_file_open_codes_the_group_stock_rule' (508179) panicked at crates/rs_cam_core/tests/cut_direction_matches_transform_g_lateralsign.rs:238:17:`
  `the named rule must live in /home/ricky/personal_repos/rs_cam/.claude/worktrees/bridge-cse_01X9bPqEnCfUkrbNprBjUs3d/crates/rs_cam_core/src/session/compute/simulation.rs`

### feed_explanation_snapshot_b3
- failing tests: the_commanded_feed_per_tooth_is_never_compared_to_the_band, the_gate_observation_and_the_band_are_now_the_same_quantity, the_predicted_feed_factor_is_live, the_sample_engagement_arc_cancels_out_of_the_gate_observation, the_gate_observation_reconciles_to_the_two_labelled_stages
- first panic: `thread 'the_commanded_feed_per_tooth_is_never_compared_to_the_band' (510495) panicked at crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs:394:33:`
  `row publishes ae_min`

### heatmap_two_arc_divergence_a1
- failing tests: the_retired_measure_still_reproduces_the_defect
- first panic: `thread 'the_retired_measure_still_reproduces_the_defect' (540649) panicked at crates/rs_cam_core/tests/heatmap_two_arc_divergence_a1.rs:508:5:`
  `assertion `left != right` failed: the retired measure must keep reproducing F-HEATMAP; if it stops, the fixture no longer demonstrates what was fixed and must be re-derived, not deleted`

### machine_kinematics_cycle_time_f034
- failing tests: cycle_time_calibrated_against_shapeoko_reference
- first panic: `thread 'cycle_time_calibrated_against_shapeoko_reference' (541203) panicked at crates/rs_cam_core/tests/machine_kinematics_cycle_time_f034.rs:387:5:`
  `F-034: model predicted 1650.0s vs measured 827.0s (ratio 1.995) — outside tolerance [0.30, 1.6]. max_feed used: 10000 mm/min. The MEASURED constant is a stale pre-F-038 wall-clock; re-bench the current path before tightening further (planning/cycle_time_rebench.md).`

### scallop_trace_survives_relink_g_linktrace
- failing tests: the_link_stage_keeps_the_scallop_trace_and_moves_no_motion
- first panic: `thread 'the_link_stage_keeps_the_scallop_trace_and_moves_no_motion' (612944) panicked at crates/rs_cam_core/tests/scallop_trace_survives_relink_g_linktrace.rs:179:5:`
  `assertion `left == right` failed: G-LINKTRACE: emitted motion CHANGED. This fix is a provenance fix          and must not move a toolpath byte; the digest was captured on the          pre-fix binary.`

### setters_are_crate_private_wp15b
- failing tests: neither_scan_is_vacuous, every_session_setter_is_declared_crate_private
- first panic: `thread 'neither_scan_is_vacuous' (613136) panicked at crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs:527:5:`
  `the declaration scan must find at least 50 `&mut self` methods inside `impl ProjectSession` blocks. I read 12`

### setters_have_rows_wp15a
- failing tests: every_public_setter_has_a_command_row
- first panic: `thread 'every_public_setter_has_a_command_row' (613146) panicked at crates/rs_cam_core/tests/setters_have_rows_wp15a.rs:366:5:`
  `WP15a (§25 ruling 1): every public `ProjectSession` setter is reachable through `ProjectSession::apply`. Add a registry row to `for_each_command!` whose `apply` arm delegates to the setter; the setter keeps the invalidation rule, and the arm never re-derives a stale set. I read 1 setter(s) that no r`

### simulation_issue_channel_m1
- failing tests: engagement_is_unmeasurable_below_the_fresh_material_floor
- first panic: `thread 'engagement_is_unmeasurable_below_the_fresh_material_floor' (613791) panicked at crates/rs_cam_core/tests/simulation_issue_channel_m1.rs:430:5:`
  `shallow arm read peak radial engagement 0.07692307692307683; expected an identical zero because every cell held < 0.05 mm above the cutter`

### tapered_width_model_parity_c3
- failing tests: the_c3_feed_delta_is_pinned
- first panic: `thread 'the_c3_feed_delta_is_pinned' (619644) panicked at crates/rs_cam_core/tests/tapered_width_model_parity_c3.rs:309:9:`
  `doc=0.05: feed 694.1803681263457 left its pinned value 1200. Since G-CHIPTHIN-HALFFIX (2026-08-19) that value is also the pre-C3 one (1200) — effective diameter no longer reaches the feed, so if these have diverged again, something is multiplying the feed by a geometry term. See the Step 5 note in f`

### the_test_doors_are_gated_fld0405
- failing tests: no_product_file_names_a_test_door
- first panic: `thread 'no_product_file_names_a_test_door' (619958) panicked at crates/rs_cam_core/tests/the_test_doors_are_gated_fld0405.rs:433:5:`
  `FLD-04 + FLD-05: the four cache-stats types, their `stats()`, `reset_stats` and `cache_len` readers, `reach_map_for_mesh`, `reset_drop_call_count`, `TierMap::label_at` and `GridZ::is_covered` are test doors behind the `test-support` feature. A product file that names one in code either breaks the de`

### vendor_sidebyside_chipload
- failing tests: the_recommendation_is_the_transferred_band_midpoint_times_the_derate_stack, vendor_sidebyside_chipload_spotcheck
- first panic: `thread 'the_recommendation_is_the_transferred_band_midpoint_times_the_derate_stack' (627304) panicked at crates/rs_cam_core/tests/vendor_sidebyside_chipload.rs:374:5:`
  `expected at least 4 unclamped vendor-banded probes to exercise the identity, got 2`
