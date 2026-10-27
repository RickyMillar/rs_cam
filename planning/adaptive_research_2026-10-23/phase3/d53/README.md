# D53: a rest consumer with a larger tool refuses; hand-written Move Debug

Tree: worktree `adaptive-head`. Not committed.

- `tree_vs_607e52730.patch` (the requested base), md5 `3f69789a91849d7cd1c6cb9085d7bd97`. It holds the D50 addendum and D53.
- `tree_vs_dabf258d3.patch`, md5 `1373903a73d857b941cae86010e9b428`. It holds D53 only. The lead committed the addendum as `dabf258d3` during this work.

## 1. The refusal

- The rule is `ProjectSession::refuse_larger_rest_consumer` (`session/compute/generation.rs:524`). It refuses a `DerivedRestRegions` consumer with `Center` or `Outside` containment whose tool radius is above the radius of the source op's tool.
- The radius is the built cutter's diameter over two (`session_cutter`). The containment polygons use the same value (`ResolvedGenInputs::boundary_centre`).
- `Inside` containment never refuses, because it insets the regions by the consumer's own radius.
- A tool that does not resolve is left to its own refusal.
- Path: it is called in the existing `DerivedRestRegions` generation precondition (`session/compute.rs:2499-2513`), right after `resolve_derived_rest_region_polys`. That precondition runs before any geometry work, and it refuses through `SessionError::OperationFailed`, the type the precondition already uses for a missing or empty source.
- The text names:
  - the consumer, the source and the containment;
  - both radii, for example "4.000 mm" and "3.000 mm";
  - the overcut, the difference of the radii;
  - the two fixes: "Set the boundary containment to Inside, or use a tool no larger than the tool of '<source>'".

Sentry: `crates/rs_cam_core/tests/a_larger_rest_consumer_tool_refuses_d53.rs`. It needs no research arm (the regions come from the generic rest analysis) and it is not ignored. It uses the D47 channel pair with a Ø6 source.
- A Ø8 consumer with `Center`, and one with `Outside`: refused. The text holds both radii, "Inside" and "no larger than", and the consumer has no result.
- A Ø8 consumer with `Inside`, a Ø6 with `Center` and a Ø4 with `Center`: each generates.

Red without the call (`sentry_red_without_refusal.txt`): "Center: a larger tool was not refused: Ok(())". Green with it.

## 2. Move Debug

This was already done in the D50 addendum, committed in `dabf258d3`. `crease_own_region_pr6b` passes 3/3 and `finish_resolution_policy_pr3` passes 10/10 on this tree. Before the port they failed 1/3 and 3/10 (D50 README, Addendum A).

## Gates (all through `scripts/cargo_lane.sh`, in the foreground)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features with and without `research` | ok |
| `adaptive3d_emission_byte_parity`, plain and research | 5/5 and 5/5 |
| D53 sentry | 1/1 (red without the call) |
| `crease_own_region_pr6b`, `finish_resolution_policy_pr3` | 3/3, 10/10 |
| D47; f140; `generic_rest_routing_pr7`, `region_cap_honesty_f3`, `rest_region_tip_radius_dilation_f1`, `dependency_edges_are_the_walker_dep1` | all ok |
| D44 x2, D48, D50 (arm env, `--ignored`) | 1/1 each |
| `rs_cam_core --lib`, plain and research | 2701 and 2752 ok |
| `rs_cam_viz --lib -- rest` (`-j 2`) | 23 ok |

Shipped output is unchanged: the rule only refuses, before generation.
