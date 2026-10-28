# W1 pre-registration (helical Waterline, D69 §2)

Written before the wider test run, on the tree of the W1 patch (HEAD 5974e642c + W1).

## What changes

- `WaterlineConfig::continuous` defaults to `true`. With it on, the standalone Waterline op joins each chain of whole
  rings over consecutive levels into one entry. With no chain, the output is the stacked form move for move.
- A chain run gets two `RapidOrderBarrier` spans (start, end). The depth-run spans do not change.
- The UnifiedFinish very-steep band and Steep and Shallow do not read the dial: no change.
- The schema of `continuous` gains a help text.

## Goldens and fixtures that may change (re-bless only these)

1. `tests/fixtures/op_golden_g_opgolden.toml`, rows `waterline/fresh` and `waterline/rest` (the cap fixture: nested
   rings at each level, so chains form).
2. `tests/fixtures/perf_golden_sim_metrics_3d.json`, the Waterline entry only (if the test reads it under its feature).

## Tests that build the Waterline op and may change behaviour (no golden; a red here is examined, not re-blessed)

`every_op_conforms_w4`, `air_cut_family_calibration_w5bf4`, `a_rest_pocket_cuts_only_below_the_prior_floor_g_restlevels`,
`depth_beyond_stock_core_g_depthstockcore`, `finishing_defaults_have_no_ramp_entry_r10`,
`link_counters_visible_g_linkvisible`, `one_link_ceiling_for_every_op_g_linkceil`, `pencil_tip_float_channel_d1`,
`reach_tolerance_source_p5_1`, `retract_intent_move_type_census_w6`, `waterline_auto_ladder_is_model_top_to_bottom_r3`,
`every_op_states_its_capabilities_w2_4` (schema only), `capability_link_moves_safety`.

## Expected unchanged

Every other golden. The Waterline ladder finding, the level count and the rest tally do not change.
