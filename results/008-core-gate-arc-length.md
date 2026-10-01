---
id: 008
state: done
commit: 0015dcfcb8a4e5df620bbeae7ed361fe6c0b9e80
started: 2026-10-02T02:20:19Z
finished: 2026-10-02T03:00:13Z
---
## Summary

| Suite | Passed | Failed | Ignored | Wall |
|---|---|---|---|---|
| core gate (heavy-tests,research,test-support) | 4494 | 20 (14 targets) | 312 | 35:58, max RSS 2.6 GiB |
| rs_cam_viz | 1036 | 0 | 3 | — |
| rs_cam_cli | 58 | 0 | 0 | — |

NEW against 001/003: ONE target, `kinematics_per_axis_rate_p1`
(one_shared_pass_leaves_both_integrators_bit_identical_edg07): total_s and
cutting_s moved; rapid_s, entry_s, linking_s, retract_s, unknown_s stay
bit-identical. That is the arc-length change and nothing else in that pin.

The other 13 targets were already red in 001 (5b34e710) and 003. I did not
run fd06f407 itself, so "pre-existing" means "red at 5b34e710 and on the
kernel-fix commit". machine_kinematics_cycle_time_f034 is in that set,
but its VALUE matters for this change: it now reads model 1619.6 s vs
measured 827.0 s (ratio 1.958, tolerance [0.30, 1.6]).

## Every failing test, with its assertion lines
### the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum
      EXHIBIT: the pre-conversion 'safe' feed 0.18 mm/tooth is 1.42x the band maximum 0.1270 mm/tooth. The pre-fix gate reported Within.
    thread 'the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum' (2834255) panicked at crates/rs_cam_core/tests/chipload_formula_calibration.rs:289:5:
    the exhibit is only worth keeping if the overshoot is large; got 1.42x

### production_unified_finish_output_is_byte_identical
    PR-6b FP taper: moves 957 hash 0x852bad70cb78873c
    PR-6b FP ball: moves 660 hash 0xea1a57708b12fb52
    thread 'production_unified_finish_output_is_byte_identical' (2843478) panicked at crates/rs_cam_core/tests/crease_own_region_pr6b.rs:592:5:
    H2.4 must not move a single emitted move -- taper: got (957, 0x852bad70cb78873c), pinned (951, 0x4b4666eba3afa7ab); ball: got (660, 0xea1a57708b12fb52), pinned (659, 0xb506292bc14e7e38)

### no_session_file_open_codes_the_group_stock_rule
    thread 'no_session_file_open_codes_the_group_stock_rule' (2843537) panicked at crates/rs_cam_core/tests/cut_direction_matches_transform_g_lateralsign.rs:238:17:
    the named rule must live in /home/ricky/personal_repos/rs_cam/.claude/worktrees/job008/crates/rs_cam_core/src/session/compute/simulation.rs

### the_sample_engagement_arc_cancels_out_of_the_gate_observation
    thread 'the_sample_engagement_arc_cancels_out_of_the_gate_observation' (2848132) panicked at crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs:394:33:
    row publishes ae_min

### the_gate_observation_reconciles_to_the_two_labelled_stages
    thread 'the_gate_observation_reconciles_to_the_two_labelled_stages' (2848130) panicked at crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs:394:33:
    row publishes ae_min

### the_gate_observation_and_the_band_are_now_the_same_quantity
    thread 'the_gate_observation_and_the_band_are_now_the_same_quantity' (2848129) panicked at crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs:394:33:
    row publishes ae_min

### the_commanded_feed_per_tooth_is_never_compared_to_the_band
    thread 'the_commanded_feed_per_tooth_is_never_compared_to_the_band' (2848128) panicked at crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs:394:33:
    row publishes ae_min

### the_predicted_feed_factor_is_live
    thread 'the_predicted_feed_factor_is_live' (2848131) panicked at crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs:394:33:
    row publishes ae_min

### the_retired_measure_still_reproduces_the_defect
    thread 'the_retired_measure_still_reproduces_the_defect' (2882315) panicked at crates/rs_cam_core/tests/heatmap_two_arc_divergence_a1.rs:508:5:
    assertion `left != right` failed: the retired measure must keep reproducing F-HEATMAP; if it stops, the fixture no longer demonstrates what was fixed and must be re-derived, not deleted
      left: Within
     right: Within

### one_shared_pass_leaves_both_integrators_bit_identical_edg07
    thread 'one_shared_pass_leaves_both_integrators_bit_identical_edg07' (2882954) panicked at crates/rs_cam_core/tests/kinematics_per_axis_rate_p1.rs:506:5:
    assertion `left == right` failed: every CycleTimeBreakdown field must stay bit-identical across the EDG-07 extraction
      left: [("total_s", 4619452994175872772), ("rapid_s", 4608409862295016789), ("cutting_s", 4610461279846029822), ("entry_s", 4609694653344569001), ("linking_s", 4604986568125309581), ("retract_s", 4604371631916866981), ("unknown_s", 4606244
     right: [("total_s", 4618988817251386535), ("rapid_s", 4608409862295016789), ("cutting_s", 4608604572148084880), ("entry_s", 4609694653344569001), ("linking_s", 4604986568125309581), ("retract_s", 4604371631916866981), ("unknown_s", 4606244

### cycle_time_calibrated_against_shapeoko_reference
    F-034 REBENCH: model predicted 1620s vs measured 827s (ratio 1.958)
    thread 'cycle_time_calibrated_against_shapeoko_reference' (2883116) panicked at crates/rs_cam_core/tests/machine_kinematics_cycle_time_f034.rs:387:5:
    F-034: model predicted 1619.6s vs measured 827.0s (ratio 1.958) — outside tolerance [0.30, 1.6]. max_feed used: 10000 mm/min. The MEASURED constant is a stale pre-F-038 wall-clock; re-bench the current path before tightening further (planni

### the_link_stage_keeps_the_scallop_trace_and_moves_no_motion
    G-LINKTRACE stage ON : items 25 (move-linked 25) · regions 1 · rings 23 · fragments Some(23) · rotated Some(0) · at_depth Some(22) · digest b5906eb187c207c6
    thread 'the_link_stage_keeps_the_scallop_trace_and_moves_no_motion' (2944925) panicked at crates/rs_cam_core/tests/scallop_trace_survives_relink_g_linktrace.rs:179:5:
    assertion `left == right` failed: G-LINKTRACE: emitted motion CHANGED. This fix is a provenance fix          and must not move a toolpath byte; the digest was captured on the          pre-fix binary.
      left: 13083078626277197766
     right: 4977314096352170135

### every_session_setter_is_declared_crate_private
    thread 'every_session_setter_is_declared_crate_private' (2945008) panicked at crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs:321:5:
    at least 50 setters must survive the two allowlists, or arm 1 asserts nothing. I read 7

### neither_scan_is_vacuous
    thread 'neither_scan_is_vacuous' (2945009) panicked at crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs:527:5:
    the declaration scan must find at least 50 `&mut self` methods inside `impl ProjectSession` blocks. I read 12

### every_public_setter_has_a_command_row
    thread 'every_public_setter_has_a_command_row' (2945114) panicked at crates/rs_cam_core/tests/setters_have_rows_wp15a.rs:366:5:
    WP15a (§25 ruling 1): every public `ProjectSession` setter is reachable through `ProjectSession::apply`. Add a registry row to `for_each_command!` whose `apply` arm delegates to the setter; the setter keeps the invalidation rule, and the ar
      /home/ricky/personal_repos/rs_cam/.claude/worktrees/job008/crates/rs_cam_core/src/session/rest_stock.rs:348: drop_out_of_date_rest_results

### engagement_is_unmeasurable_below_the_fresh_material_floor
    === M1 measurability floor (FRESH_MATERIAL_THRESHOLD_MM = 0.05) ===
    shallow 0.02 mm: air 92.4% of total runtime, avg engagement 0.0000, peak radial 0.0769, peak removed height 0.0200 mm, removed volume 62.9 mm3
    deep    2.00 mm: air 35.0% of total runtime, avg engagement 0.3072, peak radial 0.9010, peak removed height 1.9800 mm, removed volume 6237.1 mm3
    thread 'engagement_is_unmeasurable_below_the_fresh_material_floor' (2945980) panicked at crates/rs_cam_core/tests/simulation_issue_channel_m1.rs:430:5:
    shallow arm read peak radial engagement 0.07692307692307683; expected an identical zero because every cell held < 0.05 mm above the cutter

### the_c3_feed_delta_is_pinned
    thread 'the_c3_feed_delta_is_pinned' (2953054) panicked at crates/rs_cam_core/tests/tapered_width_model_parity_c3.rs:309:9:
    doc=0.05: feed 694.1803681263457 left its pinned value 1200. Since G-CHIPTHIN-HALFFIX (2026-08-19) that value is also the pre-C3 one (1200) — effective diameter no longer reaches the feed, so if these have diverged again, something is multi

### no_product_file_names_a_test_door
    thread 'no_product_file_names_a_test_door' (2953335) panicked at crates/rs_cam_core/tests/the_test_doors_are_gated_fld0405.rs:433:5:
    FLD-04 + FLD-05: the four cache-stats types, their `stats()`, `reset_stats` and `cache_len` readers, `reach_map_for_mesh`, `reset_drop_call_count`, `TierMap::label_at` and `GridZ::is_covered` are test doors behind the `test-support` feature
      adaptive3d/region_map.rs:184: pub fn label_at(&self, row: usize, col: usize) -> u16 {
      adaptive3d/region_map.rs:198: let order = self.label_at(row, col);
      adaptive3d/region_map.rs:200: while col < self.cols && self.label_at(row, col) == order {

### the_recommendation_is_the_transferred_band_midpoint_times_the_derate_stack
    thread 'the_recommendation_is_the_transferred_band_midpoint_times_the_derate_stack' (2963247) panicked at crates/rs_cam_core/tests/vendor_sidebyside_chipload.rs:374:5:
    expected at least 4 unclamped vendor-banded probes to exercise the identity, got 2

### vendor_sidebyside_chipload_spotcheck
    === A  Ø3.0 2F flat / Ipe (Janka 3510) / pocket rough / DOC 0.6 ===
      NO vendor band (matched_lut_row or chipload_bounds absent)
    === B  Ø6.0 2F flat / white oak (1360) / contour finish / DOC 12.0 (2xD) ===
      raw ratios           = D 0.944882  Janka 1.066176   (extrapolated=false)
      applied scales       = D 0.972192  Janka 1.032558   total 1.003845
      warning: FeedRateClamped { requested: 4589.579358834484, actual: 4000.0 }
    === C  Ø6.0 2F flat / hard maple (1450) / pocket rough / DOC 4.0 ===
      NO vendor band (matched_lut_row or chipload_bounds absent)
    === D  Ø1.5 2F ball / radiata pine (710) / parallel finish / DOC 0.3 ===
      raw ratios           = D 1.500000  Janka 0.845070   (extrapolated=false)
      applied scales       = D 1.280606  Janka 0.919277   total 1.177232
    === E  Ø12.7 2F flat / white oak (1360) / pocket rough / DOC 6.0 ===
      NO vendor band (matched_lut_row or chipload_bounds absent)
    === F  Ø3.0 2F flat / 6061-T6 aluminium / pocket rough / DOC 1.5 ===


Full log on the runner's PC: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam/065a14e8-82c6-476f-9331-96d953a0dcb4/scratchpad/job008.log
