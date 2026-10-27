# Removal step R5: the slice keeps only its Geodesic Adaptive branches

Date: 2026-10-28. Branch `geodesic-integration`, base `7503fa3f1` (R4).
The implementer did not commit. Design: `removal_design.md` §1.2, §5.

Patch: `stepR5_tree_vs_7503fa3f1.patch`, md5
`999b24e484c599206138c93c54a86c48`. 4 files (+88/-263). No planning file.
`git apply --check -R` passes.

## 1. The gates

`is_geodesic` is deleted. Each of its uses kept its geodesic branch:

| Site (`adaptive3d/clearing.rs`) | Change |
|---|---|
| `plan_one_outline` (4 gates) | the geodesic plan only: no plan → `leave_piece`; the agent/spiral call and its `strategy`, `debug` and `slice_eng_2d` parameters go; the C22a re-replay runs whenever the filter is on (its `WithinPassLoad` condition is now always true); `feed_caps` always |
| `clear_one_region`, the F-038 forecast skip | `leave_piece` always |
| `grown_outlines` | no early `None` |
| `plan_and_emit_region`, the air read | the disc read always (`max_top_z_in_disc`); the centre read `sample_stock_top_at` is deleted with its last caller |
| `admits_capped_split` | the `strategy` parameter goes (the slice runs only the arm; CP has no slice) |
| the slice's 2D params | `keep_down_links: WithinPassLoad` always |
| `waterline_cleanup` (shared with CP) | the gate becomes "the level has left pieces": only the arm leaves pieces, so a CP run reads none and skips the check, as before |

The strategy matches outside the slice stay (`effective_step_split`,
`effective_keep_down_route`, `emission_chord_tolerance`, the boundary
reach in `path.rs`): they choose between CP and Geodesic Adaptive.

## 2. The planner-engagement plumbing

ContourSpiral was the only 3D writer of `Produced::planner_engagement`.
Gone: the `slice_eng_2d` lookup and push in `plan_and_emit_region`,
`quantize_coord`, `LevelEmission::planner_eng`, the `planner_eng`
parameter of `clear_z_level_agent_2d_slice` and of the level loop in
`path.rs`, `Adaptive3dSegmentsResult::planner_engagement`, and the field
and log entry in the output door (`mod.rs`). `Produced::planner_engagement`
stays (the 2D Adaptive op writes it) and is empty on 3D Rough, as the
W6.5 digests already show (R2). The `too_many_arguments` expectation of
`plan_and_emit_region` went unfulfilled and is removed.

## 3. Tests

The D65 admission unit test loses its strategy argument and its CP case
(CP never reaches the rule); its other six assertions are unchanged.

## 4. Gates (Geodesic and CP byte-identical)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, clippy workspace all targets, full features | ok |
| `adaptive3d_emission_byte_parity` | 6/6, no bless (CP ×3, Geodesic ×2) |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1, no bless |
| the s7_4 geometry pins, the cap census, the C12 judge (plain), the W6.5 digests, D44 ×2, D48, D50, boundary parity, By Area ×2, global gate, pieces, plan-sim gap, entry stock, coverage, `folder_maps_name_real_files_f140` | 19 binaries ok |
| F-038 coalescing, F-038b keep-down, never-ahead | ok (131 s, 46 s, 97 s debug) |
| core `--lib` filter `adaptive3d` | 74 ok, 7 ignored |
