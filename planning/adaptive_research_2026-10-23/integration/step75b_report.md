# Integration step 7.5b: rest regions in Z (D50 + addendum) and the larger-tool refusal (D53)

Date: 2026-10-28. Branch `geodesic-integration`, base `c09e0e238` (the
D46 port, after 7.4c). The implementer did not commit.

Patch: `step75b_tree_vs_c09e0e238.patch`, md5
`1164c3ddabbb61e30aad1e8ebc2f8cd0`. It holds 33 files: code, tests,
`FEATURE_CATALOG.md` and two folder maps. No planning file is in it.
`git apply --check -R` passes. D55 is not ported (lead ruling).

## 1. What changed (R 607e52730 D50, dabf258d3 addendum, 3682d1dcc D53)

| File | Change |
|---|---|
| `trace/produced.rs` | `Produced::rest_regions_top_z: Option<f64>` with its doc; the datum move shifts it by `shift.z` |
| `adaptive3d/rest_regions.rs` | `slab_top_z`; `left_regions` returns the regions and the highest slab top of a piece that meets them |
| `adaptive3d/path.rs` | each left piece carries its slab top; `rest_capped_level_top` (a ceiling on the anchor, remaining stock only, never at or below the floor) and `rest_ceiling_keeps_doc` (no cell inside the centre containment takes more than one Depth/Pass on the first level, or the anchor stays) |
| `adaptive3d/mod.rs` | `Adaptive3dDepth::rest_top_z`; the output door publishes `rest_regions_top_z` |
| `compute/execute.rs`, `execute/volumetric.rs`, `execute/rough_3d.rs` | `ExecutionContext::rest_top_z` → `VolumetricInputs::rest_top_z` → `Adaptive3dDepth::rest_top_z`; with no demand the top is dropped with the regions |
| `session/compute.rs`, `session/compute/generation.rs` | `ResolvedGenInputs::rest_top_z` from `derived_rest_top_z` (an enabled `DerivedRestRegions` boundary only); D53 `refuse_larger_rest_consumer` in the generation preconditions |
| `adaptive3d/tests.rs` | the addendum unit tests `a_rest_top_lowers_the_start_and_keeps_the_floor_d50`, `a_rest_ceiling_keeps_one_depth_per_pass_d50` (R as is) |
| 20 test files and `path.rs` | `rest_top_z: None` in each `Adaptive3dDepth` literal |
| `adaptive3d/CLAUDE.md`, `session/CLAUDE.md` | the D50 rule and the D50/D53 sentries; 40 lines each |
| `FEATURE_CATALOG.md` | the rest-analysis line: the top, the floor and Depth/Pass guard, the refusal |
| two tests (new) | §2 |

Deviation from R: on M `ToolpathConfig::tool_id` is `Option<ToolId>`
(S11), so `refuse_larger_rest_consumer` resolves the radius through
`tool_id.and_then(|id| self.tool_by_id(id))`. A missing tool is left to
its own refusal, as in R.

## 2. Tests

| Test | Source | Result |
|---|---|---|
| `a_rest_consumer_starts_at_the_left_stock_top_d50` | R 3682d1dcc; the source is `GeodesicAdaptive` (was AgentSearch under the arm env), no `ignore` | ok, 6 s: 2 regions, left stock top 7.000 |
| `a_larger_rest_consumer_tool_refuses_d53` | R 3682d1dcc as is (the source is AgentSearch, whose regions come from the generic analysis; no env) | ok, 12 s |
| `--lib adaptive3d::tests::a_rest_top_lowers_the_start_and_keeps_the_floor_d50`, `..a_rest_ceiling_keeps_one_depth_per_pass_d50` | R dabf258d3 | ok |

Defects injected (then restored):
- the ceiling never applied: D50 is red, "the consumer cuts before its
  floor pass, at Z [7.0, …]";
- the refusal call removed: D53 is red, "Center: a larger tool was not
  refused: Ok(())".

## 3. Parity

No consumer: `rest_top_z` is `None` on every shipped path, and
`rest_regions_top_z` is dropped with the regions. Byte parity 6/6 and
opgolden 1/1, no re-bless.

## 4. Gates (all through the worktree `scripts/cargo_lane.sh`)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, full clippy (heavy-tests, research, test-support) | ok |
| byte parity, opgolden | 6/6, 1/1 |
| the adaptive3d sentries (12 binaries), the census, the s7_4 pins | ok |
| D44 ×2, D48, D47, `generic_rest_routing_pr7`, `folder_maps_name_real_files_f140` | ok |
| core `--lib` filters `adaptive3d`, `session::compute`, `rest`, `produced` | 208 ok, 10 ignored |
| `command_registry_completeness`, `mutation_paths_invalidate_alike_p0`, `a_retired_boundary_source_is_named_p2_5` | ok |

## 5. Open (design §3.1)

- One top for the whole set: a region far below the highest piece starts
  too high and cuts air to its own slab. A per-region top is the exact
  form; a later item unless D10 shows the air time matters.
- The "body + CP edges" template (Q-T) must pick an edge tool no larger
  than the rough's, or use `Inside` containment; D53 refuses otherwise.
