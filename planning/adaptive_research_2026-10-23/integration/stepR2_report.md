# Removal step R2: the config layer and the load rule (refuse at load)

Date: 2026-10-28. Branch `geodesic-integration`, base `49a63308d` (R1).
The implementer did not commit. Design: `removal_design.md` §3.2, §5.

Patch: `stepR2_tree_vs_49a63308d.patch`, md5
`a453bea375278b0d694690aa40921f8d` (re-frozen after the lead's answers;
`790ed293…` is void). 25 file entries (24 tracked, +267/-335, and the new
sentry). No planning file. `git apply --check -R` passes. All gates green.

## 1. What changed

| Place | Change |
|---|---|
| `compute/operation_configs.rs` | `ClearingStrategy` keeps `ContourParallel` and `GeodesicAdaptive`; the doc names the removal and the refusal. The serde round trip covers the two values and proves the three removed strings do not deserialise |
| `session/project_file.rs` | three `RETIRED_VALUES` rows on `setups[].toolpaths[].operation.params.clearing_strategy` (`adaptive`, `agent_search`, `contour_spiral`), one replacement text ("…the 3D Adaptive clearing strategies are "geodesic_adaptive" and "contour_parallel". Set clearing_strategy to one of them"); `retired_param_value` for the parameter door |
| `session/compute/params.rs` | `set_toolpath_param` refuses a retired value with `InvalidParam("<param> = "<value>" is retired. <replacement>.")` before serde. MCP `set_toolpath_param` and a job file (the CLI applies each job key through this setter, `rs_cam_cli/src/job.rs:1270`) get the fix text, not "unknown variant" |
| `adaptive3d/mod.rs` | the three `From` arms go. The planner enum keeps all five variants until R3/R4 (reachable from tests only) |
| `compute/catalog.rs` | the 3D Rough capability arm (`planner_uniform_engagement` for ContourSpiral) goes; 3D Rough joins the arms that keep the row |
| `machine/strategy_advisor.rs` | the label and constant-engagement arms of the removed values go; the unit tests use Geodesic Adaptive as the constant-engagement candidate |
| `session/compute.rs` | `ADVISOR_CANDIDATE_STRATEGIES = [ContourParallel, GeodesicAdaptive]` (ruled) |
| `feeds/suggest/adaptive_entry.rs`, `feeds/rationale.rs` test, `feeds/suggest/tests.rs` | heuristic B recommends `geodesic_adaptive` (ruled) |
| `rs_cam_viz/.../surface_3d.rs`, `registry.rs`, `toolpath_panel.rs` | the combo offers two values; the ContourSpiral "Optimal load" slider and its helpers go; the stepover pill is the only stepover control; `OpDrawCtx::tool_radius` (read only by the slider) goes |
| `FEATURE_CATALOG.md` | the 3D Adaptive row: two strategies, the refusal, the advisor and Suggest rulings, the sentry |

## 2. Tests changed

| Test | Change |
|---|---|
| `a_retired_clearing_strategy_is_named_r2` (new) | control: CP and Geodesic save as values and load; a text with the three removed values refuses with one `RetiredKeys` naming all three and the fix; the parameter door refuses each with the fix and not "unknown variant"; a kept value still sets. Red when one row is broken ("one refusal must name every removed value") |
| `every_op_keeps_its_toolpath_g_opgolden` | the rows `adaptive3d/adaptive`, `by_area_helix_spiral`, `ramp_agent` and their setters go (bless: 42 lines removed, nothing added or changed). Their coverage is in the R1 rows `by_area_helix_cp`, `ramp_geodesic` |
| `the_volumetric_wrapper_keeps_its_produced_w6_5` | arms `by_area_helix_cp`, `ramp_geodesic` replace the two removed arms. The bless renames the two rows; `fresh` and `rest` are unchanged; `by_area_helix_cp` keeps the same area hash (`696d58d1…`) with 0 engagement samples (no 3D strategy writes planner engagement now). The non-vacuity check reads the areas and asserts 0 samples on every arm |
| `every_op_states_its_capabilities_w2_4` | 3D Rough: no value holds a uniform engagement (CP, Geodesic); the 2D arms unchanged |
| `adaptive3d_plans_every_slice_piece_g_adapt3dpieces` | the AgentSearch and ContourSpiral tests become one Geodesic Adaptive test (§4) |
| `adaptive3d_planner_never_ahead_of_emitted_path` | its session arm runs Geodesic Adaptive (was AgentSearch) |
| `a_larger_rest_consumer_tool_refuses_d53` | the source rough is Geodesic Adaptive (was AgentSearch) |
| `strategy_advisor_smoke` (heavy) | the chosen strategy is CP or Geodesic Adaptive. Run with the operator's approval (§4) |
| `session/compute/tests.rs::advisor_modulates_candidate_feeds_before_timing` | its candidate is CP (was ContourSpiral); its doc says the subject is the modulation step, not the strategy (lead ruling). Geodesic Adaptive took 207 s on this terrain session in debug (7.4b) |

## 3. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, clippy workspace all targets, full features | ok |
| `adaptive3d_emission_byte_parity` | 6/6 (CP and Geodesic byte-identical; the removed cases still run on the planner enum) |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1 after the bless above |
| `schema_enum_values_g_schemaenum`, `a_value_outside_its_domain_is_refused_at_every_door_w6b`, `every_project_table_refuses_an_unknown_key_f034`, `a_retired_boundary_source_is_named_p2_5`, `every_op_conforms_w4`, `wanaka_suggest_integration`, the volumetric wrapper, capabilities, D53, never-ahead | ok |
| core `--lib` filters `feeds::`, `strategy_advisor`, `operation_configs`, `catalog`, `project_file`, the advisor test | 295 ok |
| `rs_cam_cli` (all) | 0 failed |
| viz `-j 2`: `mcp_wire_surface_pin` (unchanged, 16/16), `component_contracts_up2`, `apply_contract_a3`, `--lib` filters `round_trip`, `properties` | ok |

## 4. The lead's answers

### 4.1 The pieces sentry, re-scoped (lead ruling)

`adaptive3d_plans_every_slice_piece_g_adapt3dpieces` claims "every slice
piece is planned". An unplanned piece leaves a connected region of stock
over the target at least the size of the area the tool would clear. The
check: no 4-connected region of over-target judged cells (more than one sim
cell, `CELL_MM` = 0.5 mm, over the floor plus the leave) is larger than one
tool disc, `π R²` = 28.27 mm². Both values derive from the tool and the
sim grid; no number is taken from a run.

- Geodesic Adaptive: lobe x −1 has 1 cell at 0.374 mm over the target
  (under one sim cell, so not over-target) and lobe x +1 none; the largest
  region is 0.00 mm². Green.
- The red, on the old numbers (`the_region_check_fails_the_old_unplanned_piece`,
  new): the D2 defect left 555 of the 2209 judged cells of the +X lobe at
  5.5 mm, the core of the unplanned piece. Planted as one connected block
  of 555 cells (15 × 37) it is 138.75 mm², over 28.27 mm²: the check fails
  it. One cell passes.
- The red by injection did NOT show: with the split cut to the first piece
  (`.take(1)`) in `planner_outlines` and then in `grown_planner_outlines`,
  the Geodesic Adaptive run stayed green both times (both edits reverted;
  `git diff` of `clearing.rs` empty). On this fixture the arm's outline
  does not split, so the run does not reach the D2 mechanism. The file
  keeps the claim on the product strategy, but only the planted case
  proves that the check can fail. The D2 split itself has no live
  strategy test on this fixture now; a fixture whose grown inset splits
  would give one.

### 4.2 `strategy_advisor_smoke` (operator approval)

Run once in `release-fast` (opt-level 3, no fat LTO; the test runs under
any profile): 1/1 ok, test time 37.45 s (build and run 104 s). The
advisor's wall time per candidate, read with a temporary timer in the
candidate loop (reverted; `session/compute.rs` is back to the frozen text):
Contour Parallel 3.77 s, Geodesic Adaptive 34.14 s (test 37.98 s). The
operator will feel about 38 s for one advisor call on this terrain, 34 s
of it the Geodesic Adaptive plan. Not measured: the full `release` profile
(fat LTO); expect it a little faster than `release-fast`.

### 4.3 The advisor test's candidate

Accepted by the lead; the doc comment of
`advisor_modulates_candidate_feeds_before_timing` now says its subject is
the modulation step, not the strategy.
