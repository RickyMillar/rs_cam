# D50: a rest consumer starts at the top of the stock the source left (D48 proposal c); folder maps

Tree: worktree `adaptive-head`, base `453d52571` (D48 committed). Not committed.
Patch: `tree_vs_453d52571.patch`, md5 `cbed01a197c75342ecb029f069d02e36` (31 files, all in `rs_cam_core`).

## 1. The Z span limit (proposal c)

### What the source publishes

- `Produced::rest_regions_top_z: Option<f64>` (`trace/produced.rs:71`) holds the top Z of the stock that a 3D Rough left in `rest_regions`. It is one value for the whole set. `translated` shifts it by `shift.z`. `run_volumetric` drops it with the regions when no consumer names the op (`compute/execute/volumetric.rs:185`).
- The top is the top of the SLAB that the highest left piece leaves, not that piece's level Z. A level cuts the stock from its own Z up to the level above it. So a piece left at level z leaves the stock up to the next level above z, or up to the level anchor for the first level. `slab_top_z` (`adaptive3d/research_outer_band.rs:71`) reads this from the run's own level list. The code adds no number.
- `path.rs:1308` turns each piece's level Z into its slab top when the pieces leave the planner. `left_regions` (`research_outer_band.rs:101`) returns the highest top of a piece that lies in the clipped regions (D48 clip). So a piece that the clip removed does not raise the top.
- The rest field (pencil, generic analysis) states no top, so the value is `None` for its regions.

### What the consumer does

- Generation reads the top only when the enabled boundary is `DerivedRestRegions` (`session/compute/generation.rs:458`, `derived_rest_top_z` at `:494`). The value goes through `ExecutionContext::rest_top_z` (`compute/execute.rs:504`), `VolumetricInputs::rest_top_z` (`volumetric.rs:98`) and `Adaptive3dDepth::rest_top_z` (`adaptive3d/mod.rs:331`) to the planner.
- The rule (`adaptive3d/path.rs:967`): on remaining stock, the level plan starts at `min(anchor, rest_top_z)`. The anchor is G-LADDERANCHOR `level_anchor_z`, "the top of the material it can still cut". The published top is a ceiling on that anchor, and `step_levels` is unchanged. So the first level is the top minus the consumer's own DPP. This is your rule "drop the levels above the span's top + own DPP", derived from the anchor logic. The difference: the top here is the slab top, so the level at the top itself (which cuts nothing) is not planned.
- On FRESH stock the ceiling is not applied, and the code documents why. The planner then holds the whole stock above the top, so a first level under the top would take all of it in one pass. Both real consumers (Corne op 5, wanaka op 7) are `from_remaining_stock`.
- With no consumer, `rest_top_z` is `None` everywhere, and the plan and the output are as before. Parity is 5/5 plain and 5/5 with research.
- 20 `Adaptive3dDepth` literals (tests and internal call sites) gain `rest_top_z: None`.

### Sentry

`crates/rs_cam_core/tests/a_rest_consumer_starts_at_the_left_stock_top_d50.rs` (research and test-support, arm env, `--ignored`, as the D44 and D48 sentries are).

- Fixture: the D44 stepped pocket with two channels below the wide pocket.
  - The source leaves pieces at its lowest level only. The test asserts that the published top is 7.000, the level above the channels.
  - There are two regions, so the consumer's planner does not pre-clear the stock outside them (a single region would hide the defect).
  - The source leaves 1 mm of axial stock and the consumer leaves 0, so a skin stays on the wide floor above the slab top.
- The consumer (CP, `FromRemainingStock`, `DerivedRestRegions`) is generated after a simulation of the source.
- The test reads the consumer's level plan from its `DepthPass` spans. Two checks:
  - no pass above the floor level (Z 4) cuts;
  - no move that is not an entry or a link cuts before the floor's pass.
- Red with the ceiling off (the same tree with `path.rs:967` returning the anchor, `sentry_red_ceiling_off.txt`): 52 moves. The floor pass has plan index 2, and 12 cuts at Z 7.0 come before it. They are the waterline cleanup of an upper level, on the skin along the channel edges. This is the wanaka "pecks at levels where the piece does not exist" pattern.
- Green: 25 moves, 0 cuts before the floor pass.
- The base tree cannot run this test, because it has no `rest_regions_top_z`. So the red proof disables the one rule in this tree.
- Note: the floor pass has plan index 1 with the fix, not 0. One level above the floor (a Detect Flat shelf at Z 7, the top itself) is planned but cuts nothing. The sentry allows a level that cuts nothing.

### Not built (waits for the operator)

Per-level masks (a) and (b). The top is one value for the whole set, so a region whose pieces are only at a low level still gets the levels down from the highest piece's slab top.

## 2. A consumer tool larger than the source tool: proposal

Today (D48) the published regions are the source's tool-centre positions. A consumer with `Center` containment and a larger tool can cross the source's boundary by the difference of the radii.

Recommendation: REFUSE at generation, not only state it. A statement in a doc does not stop a cut into the holding frame. The refusal conditions:
1. the boundary is `DerivedRestRegions` with `Center` (or `Outside`) containment, and
2. the consumer's tool radius is above the source's tool radius.

The refusal names both radii and the fix: use `Inside` containment, or a tool no larger than the source's. The session already knows the source op's tool, so `Produced` needs no new field. `Inside` containment insets the regions by the consumer's own radius and is always inside. Place it next to the other generation refusals (`SessionError`, the S11 order), with its own sentry.

## 3. Folder maps (pre-existing red)

- `adaptive/CLAUDE.md` names `slab_field.rs`.
- `adaptive3d/CLAUDE.md` names `keep_down_route.rs` and `research_outer_band.rs` on an existing line, and names the D50 sentry.
- Both files have 40 lines. `folder_maps_name_real_files_f140`: 4/4.

## Gates (all through `scripts/cargo_lane.sh`, in the foreground)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features with `research` | ok |
| clippy, without `research` | ok |
| `adaptive3d_emission_byte_parity`, plain and research | 5/5 and 5/5 |
| `folder_maps_name_real_files_f140` | 4/4 (red at base) |
| D44 publish, D44 consumer, D48, D50 (arm env, `--ignored`) | 1/1 each |
| `a_skipped_op_does_not_stale_the_export_d47` | 1/1 |
| `generic_rest_routing_pr7`, `region_cap_honesty_f3`, `rest_region_tip_radius_dilation_f1`, `dependency_edges_are_the_walker_dep1` | 6, 4, 4, 9 ok |
| `rs_cam_core --lib`, plain and research | 2698 and 2749 ok |

Viz and CLI tests are not run. The patch changes only `rs_cam_core`, and the workspace clippy compiles every viz and CLI target.

## Addendum (after the lead committed D50 as `607e52730`)

Patch: `tree_vs_607e52730.patch`, md5 `6265e0c92956a486603b88ed0a2e1ffd`. 4 files: `toolpath.rs`, `adaptive3d/path.rs`, `adaptive3d/tests.rs` and the D50 sentry. Not committed.

### A. Hand-written `Debug` for `Move` (from geodesic-integration `5d700703a`)

- The cause of the pinned-hash drift: D34 (`7b7a8df6c`) added `Move::feed_cap` under a derived `Debug`. Every move then printed `feed_cap: None`, and the Debug-hash fingerprints moved.
- The port: `Move` keeps a hand-written `Debug` that prints `feed_cap` only when it is `Some`. The unit test `a_move_with_no_cap_prints_as_before_the_field` shows that the text equals the old derived text in the `{:?}` and `{:#?}` forms.
- Before the port: `crease_own_region_pr6b` failed 1 of 3, and `finish_resolution_policy_pr3` failed 3 of 10. After the port: 3/3 and 10/10. The known red "stale pinned hashes" was this drift, not stale pins.
- No golden file holds `feed_cap: None`. Since D34, no fingerprint test was re-blessed (the only edits are D46 feeds and the D50 field). Twenty non-heavy fingerprint tests pass, and four viz tests that read `{:?}` text pass.

### B. The top lowers only the start, never the floor (integration design §3.1)

- `rest_capped_level_top` (`path.rs`) holds the rule: on remaining stock the start is `min(anchor, rest_top)`.
- A top at or below `z_bottom` is not applied. It would leave `step_levels` no level, so the floor would move.
- Unit test `a_rest_top_lowers_the_start_and_keeps_the_floor_d50`:
  - the plan with the top ends at the same floor as without it;
  - a low top, fresh stock and no top all leave the anchor;
  - a top above the anchor does not raise the start.
- The D50 sentry now also asserts that the consumer's lowest level is its floor (Z 4).

### C. A guard found in this work: the ceiling must keep one Depth/Pass per level

- The anchor rule holds every level to one Depth/Pass, because the first level is one DPP under the highest stock.
- A top under the anchor can break that. Example: a source `stock_to_leave` above the consumer's by more than the consumer's DPP leaves a thick skin where the consumer's centre goes, and the first level would take it in one pass.
- The guard `rest_ceiling_keeps_doc` (`path.rs`) applies the ceiling only when, at every cell inside the consumer's centre containment, the stock above THE level floor (`level_floor_z`) of the first level is at most one DPP. The slack is the op tolerance. Otherwise the planner keeps the anchor and logs it.
- Cells only under the disc edge do not count. A centre beside them cuts them at its drape height with or without the top.
- Unit test `a_rest_ceiling_keeps_one_depth_per_pass_d50`: a 1.5 mm skin over the leave refuses the ceiling at DPP 1.0 and allows it at DPP 2.0. A thick block outside the containment does not count, and it counts when there is no containment.
- On wanaka op 1 and op 7 have the same leave (4.0) and DPP (5.46), so the guard should pass there. This is expected, not measured.

### D. Wanaka op 7: what one top for the whole set costs (from the fix33 logs, tree `7bf7738a1`)

- Op 1's levels are 19.54, 14.08, 8.62 and 4.0. Every left piece in both runs is at `z=4.0` (s2.0: 5 piece lines; s3.5: 57). So the published top is 8.62 for every region. A per-region top would give the same plan, and one top for the set adds no air on these runs.
- D50 removes op 7's three upper levels. Its plan becomes one level at 4.0 (`step_levels(8.62, 4.0, 5.46)`). Entries per level, from op 7's semantic trace:

| run | Z 19.54 | Z 14.08 | Z 8.62 | Z 4.00 | removed |
|---|---|---|---|---|---|
| s2.0 | 28 | 461 | 217 | 686 | 706 of 1392 |
| s3.5 | 35 | 66 | 123 | 319 | 224 of 543 |

- The 461 at Z 14.08 are the DEFECTS row's "461 plunges at Z −10.92" (14.08 − 25).
- The logs hold no per-level clock (`stdout_plain.txt` has only the level plan and the piece lines). So the seconds that D50 removes, and any air left at Z 4.0, need a re-run of fix33 on this tree. Each run takes about 30 s plus its simulation.

### Gates for the addendum (all through the lane, in the foreground)

fmt; clippy with and without research; parity 5/5 and 5/5; f140 4/4; `crease_own_region_pr6b` 3/3; `finish_resolution_policy_pr3` 10/10; D47 1/1; D44 x2, D48, D50 1/1 each (D50 red with the ceiling off: 12 cuts at Z 7.0 before the floor pass, `sentry_red_ceiling_off_addendum.txt`); `rs_cam_core --lib` 2701 plain and 2752 research; the four rest-consumer sentries; 20 fingerprint tests; 4 viz Debug-text tests.
