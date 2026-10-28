# Step G-A: Z Blend is retired

Implementer: I4. Date: 2026-10-28. Base: `5a80bd1b2`. Design:
`gui_fix_design.md` §5 (ruling D-3: `false` loads, `true` refuses), with
the review's implementation notes:

- the matcher compares the TOML value, so a boolean row matches;
- the new sentry writes `true` as a TOML boolean;
- the bless log prints the move counts before and after.

The Waterline label "Helical:" was already committed as GH (`efd34d1b0`),
so it is not in this patch.

Patch: `stepGA_tree_vs_5a80bd1b2.patch` (42 entries: 39 changed files, one
deleted fixture, and two new files). Checked with `git apply --check` on a
clean `5a80bd1b2`.

The tree also holds other agents' uncommitted work, and the patch leaves it
out:

- `stock/*`, `trace/narrate.rs` and `app/mcp/simulation.rs` (W3);
- I5's W2b edits in `session/compute.rs` and `generation.rs`.

I did not touch `session/compute.rs`, `generation.rs` or `session/mod.rs`.

## What changed

| Surface | Change |
|---|---|
| Config | `Adaptive3dConfig::z_blend` is deleted, so the `OpParams` schema drops it. `get_operation_schema` no longer lists it. |
| Engine | `Adaptive3dParams::z_blend`, the clearing context field, the pass-through (`path.rs`, `rough_3d.rs`) and the blend code (`offset_range`, `blend`, the lerp) are deleted. Each ring Z is `z_level.max(target_z)`: the old value with the blend at 0. |
| GUI | The `Z Blend:` row of the 3D Rough Advanced section is deleted. |
| Load rule | `RetiredValue::value` is now a `RetiredLiteral { Str, Bool }`. The loader compares the TOML value (`matches`) and prints it as the file writes it (`file_text`). A new row `z_blend = true` (`Bool(true)`) refuses with `Z_BLEND_REPLACEMENT`: "Removed (GUI fix G-A, operator ruling 2026-10-28): Z Blend cut deeper than one Depth/Pass. Delete the key; the op then cuts one Depth/Pass per level". `z_blend = false` loads, and serde ignores the key. |
| Setter / MCP | `retired_param_value` matches a `Bool` row for ANY value, because there is no field left to set. So `set_toolpath_param z_blend` refuses with the same text, for `true` and for `false`. |
| CLI | The `z_blend = true` line of the sweep template goes. The round-trip test counts 15 params, not 16. |
| Fixtures | `tests/fixtures/test_job.toml` loses its `z_blend = true` line (a Geodesic op, so no output change). The `false` lines of the planning and test fixtures stay; they load. |
| Tests | 24 core test files lose their `z_blend: false,` struct line. `param_sweep.rs` `sweep_adaptive3d_z_blend` (an ignored instrument) is deleted, with a one-line note. `end_to_end.rs` `test_adaptive3d_rapids_lift_before_xy_traverse` keeps its guard on the plain Contour Parallel run; its non-vacuity assert (at least one XY rapid) still holds. Two stale comments in `session/compute` are corrected. |
| Docs | `FEATURE_CATALOG.md`: the 3D Adaptive row states the removal and the load rule. |

## The golden

`adaptive3d_emission_byte_parity`: the case `contour_parallel_flat_blend`
becomes `contour_parallel_flat` (`detect_flat_areas` only). I blessed it
alone (`ADAPTIVE3D_BYTE_PARITY_BLESS=1`, filter
`contour_parallel_flat_emission`) and deleted the old fixture. The bless log
in the test file records the change: **1197 moves before, 1124 after**.
The design's probe measured 1124 for the Z-Blend-off run; it said 1198 for
the old dump, but the committed dump says 1197.

The other four cases do not move, and the file's 6 tests pass without the
bless. `every_op_keeps_its_toolpath_g_opgolden` passes with no re-bless:
no golden row sets Z Blend.

## Panel line count

| State (3D Rough, Cut tab) | Before | After |
|---|---|---|
| Advanced closed | unchanged | unchanged |
| Advanced open | 5 Advanced rows | 4 (Tolerance, Min Cut Radius, Detect Flat, Ordering) |

## Tests

New sentry `--test a_z_blend_true_refuses_at_load_g_a`:

1. A new save writes no `z_blend` key.
2. `z_blend = false` (a TOML boolean) loads.
3. `z_blend = true` refuses with `RetiredKeys`. The test checks the path
   `setups[0].toolpaths[0].operation.params.z_blend = true`, and that the
   text names "Depth/Pass" and "Delete the key".
4. `SetToolpathParam z_blend` refuses with the same text for `true` and
   for `false`.

## Gates (green)

All cargo runs went through `scripts/cargo_lane.sh`, in the foreground.

- `cargo clippy --no-deps -p rs_cam_core -p rs_cam_viz -p rs_cam_cli --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings`:
  clean. So every target compiles without the field.
- Core `--test` sentries, all passed:
  - `a_z_blend_true_refuses_at_load_g_a`
  - `a_retired_clearing_strategy_is_named_r2`
  - `adaptive3d_emission_byte_parity` (6)
  - `every_op_keeps_its_toolpath_g_opgolden`
  - `end_to_end -- test_adaptive3d_rapids_lift_before_xy_traverse` (the
    terrain fixture is present)
  - `a_larger_rest_consumer_tool_refuses_d53`
  - `a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10`
- Core `--features heavy-tests --test` sentries, all passed:
  `strategy_advisor_smoke`, `every_op_conforms_w4`,
  `adaptive3d_commanded_ladder`.
- `cargo test -p rs_cam_core --lib -- adaptive3d project_file catalog params session::compute::tests`:
  192 passed.
- `cargo test -p rs_cam_cli`: 67 passed.
- `cargo test -p rs_cam_viz --lib`: 606 passed.
- `--test` viz sentries, all passed: `mcp_wire_surface_pin` (the snapshot
  does not change), `mcp_authoring_surface`,
  `the_inspector_text_is_bounded_ic1`,
  `the_help_key_is_the_param_name_ui04`, `operations_registry_ui05`.
- `cargo fmt --check`: clean on my files. The only diff is I5's new
  untracked `a_rest_waterline_after_a_rough_widens_its_wall_step_w2.rs`.
