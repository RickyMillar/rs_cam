# Integration step 7.4b: Geodesic Adaptive is a product strategy

Date: 2026-10-28. Branch `geodesic-integration`, base `22557a199` (7.4a).
The implementer did not commit.

Patch: `step74b_tree_vs_22557a199.patch`, md5
`aafc3b16b59a1142d2dbecdbd2fddffb`. It holds code, tests, the new golden
and `FEATURE_CATALOG.md` only (9 files). No planning file is in it.
`git apply --check -R` passes.

## 1. What changed

| File | Change |
|---|---|
| `compute/operation_configs.rs` | `ClearingStrategy::GeodesicAdaptive` (serde `geodesic_adaptive`; the `ParamEnum` derive writes it into the W5 schema string). `clearing_strategy_serde_round_trip` covers every variant and pins the wire text `"geodesic_adaptive"` |
| `adaptive3d/mod.rs` | `From<ClearingStrategy>` maps `GeodesicAdaptive` to `ClearingStrategy3d::GeodesicAdaptive`. The dispatch (`rough_3d.rs`, `.into()`) needs no change |
| `machine/strategy_advisor.rs` | `is_constant_engagement`: yes; `strategy_label`: "Geodesic Adaptive". `ADVISOR_CANDIDATE_STRATEGIES` stays `[CP, ContourSpiral]` (design §4.5) |
| `rs_cam_viz/.../surface_3d.rs` | one combo row "Geodesic Adaptive" with hover text: body only, follow with a Contour Parallel rest op, new and not yet machine-tested. No new parameter row: the usual roughing parameters only (Q9). The combo writes `cfg.clearing_strategy` through the existing properties path; no new `Command` |
| `tests/adaptive3d_emission_byte_parity.rs` | the fifth case `geodesic_adaptive` (stay-down 8 mm, the arm's own step split and route). A move with a cap adds ` cap=<bits>` to its line; a move with none adds nothing |
| `tests/fixtures/adaptive3d_emission/geodesic_adaptive.txt` (new) | 2019 moves, 174 with a cap. Blessed for this case only |
| `tests/every_op_states_its_capabilities_w2_4.rs` | `geodesic_adaptive` joins the values with no uniform planner engagement |
| `session/compute/tests.rs` | `geodesic_adaptive_generates_through_the_session` (§3) |
| `FEATURE_CATALOG.md` | the 3D Adaptive row: the strategy, "NEW, NOT YET MACHINE-TESTED", body only, the cap rule, an older build refuses the file, the sentries |

`planner_uniform_engagement` stays false for the arm. That capability
gives the feed modulator a planner engagement at the stepover
(`session/compute.rs`, the Stage 4 comment). The session gates it to
ContourSpiral, because a strategy with real engagement over the stepover
could be over-fed. The arm's turns and departures read over the stepover,
so it keeps the shipped default (no planner engagement).

Not done (by the lead's 7.4b list): the Agent Search "legacy" hover text
(Q5 / Q-L, open), the C12 judge on the product arm (design 7.7), the
advisor candidate.

## 2. Parity

- `adaptive3d_emission_byte_parity`: 6/6. The four older fixtures are
  unchanged (`git status` shows only the new fixture). The new case and
  the determinism test pass.
- `every_op_keeps_its_toolpath_g_opgolden`: 1/1, no re-bless.

## 3. The session door

`geodesic_adaptive_generates_through_the_session` (core `--lib`, 5 s):
the terrain project with `clearing_strategy = GeodesicAdaptive`, stepover
3 mm and one level (set by `set_toolpath_param`). It generates through
`generate_toolpath` and asserts that the result carries feed caps and that
no cut leaves over its cap. With the `From` arm mapped to ContourParallel
the test is red ("the arm wrote no feed cap (5022 moves)").

## 4. Gates (all through the worktree `scripts/cargo_lane.sh`)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features (heavy-tests, research, test-support) | ok |
| `adaptive3d_emission_byte_parity` | 6/6 |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1 |
| `schema_enum_values_g_schemaenum` | 21/21 |
| `every_op_states_its_capabilities_w2_4` | 2/2 |
| `every_op_conforms_w4` | 12/12 |
| `geodesic_adaptive_keeps_the_research_geometry_s7_4`, `a_geodesic_cap_survives_every_dressup_s7_4`, `folder_maps_name_real_files_f140` | ok |
| core `--lib` filters `strategy_advisor`, `operation_configs`, `catalog`, `feeds::suggest` | 83 ok |
| core `--lib` `geodesic_adaptive_generates_through_the_session`, `clearing_strategy_serde_round_trip` | 2/2 |
| `rs_cam_cli` (all) | all ok, 0 failed |
| viz `-j 2`: `mcp_wire_surface_pin`, `component_contracts_up2`, `apply_contract_a3` | 16, 23, 2 ok; the MCP snapshot does not move |
| viz `-j 2 --lib` filter `round_trip` | 6 ok |
| viz `-j 2 --lib` filter `surface_3d` | 0 tests match (the panel has no unit test) |

## 5. Next: D10

The pre-registration is in `step74a_report.md` §8. The geometry-only op-1
numbers must equal R's exactly. The op-1 clock is expected at about
+3.5 % over R, and more than +6 % is a finding. The job file value is
`clearing_strategy = "geodesic_adaptive"`. The research runs set the step
split and the route by environment variable; the product sets them by the
strategy (`StockDepth`, `CutFeed`), so the D10 job files need no other key.
