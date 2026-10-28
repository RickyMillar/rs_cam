# W4 report: the Waterline stops at the web of a through cut (D69 §4)

Patch: `W4.patch` = `git diff e8072f172 --` 13 tracked paths + 1 new test file (14), md5
`d1d6635e04827b1d988b98fb9619018b`. `git apply --check` passes on a clean `e8072f172` checkout. Nothing is committed.
Pre-registration: `W4_PREREG.md`, approved with Q3 changed to the configs.

## What changed

- `ops/waterline.rs`: `RunFilter::outer_floor_z`. At a level at or below it (within `WATERLINE_LADDER_EPSILON`), an
  OUTER ring (no other ring of the level holds it, `ring_inside` within `sampling`, `outer_contours`) is not cut and is
  counted (`LevelRings::outer_rings_skipped`, `WaterlineLevel::outer_rings_skipped`). Hole and inner rings keep their
  levels. Both paths (stacked and helical) read it.
- `compute/toolpath_stats.rs`: `WebSource`, `WaterlineWeb`, `ThroughCutHold`; `WaterlineLadderFinding` gains `web`,
  `outer_rings_skipped`, `part_unheld`. The finding message (narration, one line) appends "stops at the web Z z (source):
  N outer ring(s) not cut" when rings are skipped.
- `compute/execute.rs`: `ExecutionContext::through_cut_hold` (`None` in `::new`).
- `compute/execute/finish_raster.rs`: the through-part test (model bottom at the stock bottom within
  `FLAT_FACE_SNAP_MM`), the web into the filter, the count, and the finding recorded again after the run.
- `diagnostics/ids.rs` + `adapters/from_generation.rs`: `geom.waterline_part_unheld` (Caution, one line: "Through cut:
  no tabbed Profile follows this Waterline, so the job does not hold the part at the end. Add a tabbed Profile on the
  outline."). The ladder finding is the detail for the GUI hover.
- `session/compute.rs`, `session/compute/generation.rs`: `ResolvedGenInputs::through_cut_hold` and
  `ProjectSession::through_cut_hold` (Waterline only), from the project (configs and resolved heights), never from a
  result:
  1. a later enabled Profile with tabs whose floor reaches the stock bottom: web = its floor + `tab_height` (the
     highest of several);
  2. else an earlier enabled roughing op (`PassRole::Roughing`) whose resolved bottom is the model bottom: web = its
     bottom + its axial leave (`model_gouge_allowance`), the lowest, when above the stock bottom;
  3. else no web.
- `rs_cam_viz/src/ui/properties/tab_badges.rs`: one row, `(i::GEOM_WATERLINE_PART_UNHELD, NO)` (I4 told first).

## One refinement of the approved pre-registration (rule 2)

Rule 2 reads only an earlier rough whose bottom is the model bottom (it cuts down to the part bottom). Without this the
restlevels claim 17 went red: its earlier Pocket stops at Z 3.5 over half the stock, and "the lowest earlier floor" made
3.5 a web, so the Waterline skipped the cap ring below 3.5. A pocket that stops above the model bottom leaves a floor,
not a web on the outline.

## Tests changed (with cause)

- `a_rest_waterline_after_a_rough_widens_its_wall_step_w2.rs` (W2b): `LEVEL_BOTTOM` 0.5 → 1.0. Its boss is a through
  part and its rough leaves a 0.5 web, so W4 skips the outer ring at 0.5 (the control read 28 of 29 levels). The file
  pins the wall step, so its ladder now stands above the web.
- `findings_transport_join_h21.rs`: the new finding fields, set to non-default values, so the join carries them.
- The W1 and W2 sentries: the new `outer_rings_skipped: 0` field of `LevelRings`.

## New sentry: `tests/a_waterline_stops_at_the_web_of_a_through_cut_w4.rs`

Fixture: a 6 mm through plate (drafted outer wall, 10 mm square hole), helical Waterline 5.5 to 0.5 by 1 mm.
1. A later tabbed Profile (tab height 2, floor at the stock bottom): web 2.0 from the Profile; 2 outer rings skipped;
   the outer ring's lowest cut is above 2.0; the hole ring keeps 0.5; no unheld Caution; the message names the web.
2. An earlier 3D Rough: web = its axial leave (0.5); 1 outer ring skipped; the hole keeps 0.5; the unheld Caution
   (one line). The lead's check: the generated rough's lowest cut is at or above that web Z.
3. A relief (plate lifted 1 mm): the same moves with and without the earlier rough; no web, no Caution.
4. A through plate alone: no web, today's ladder (outer ring down to 0.5), the unheld Caution.
Red check: with the web not passed to the filter, tests 1 and 2 fail.

## Re-bless

None. `every_op_keeps_its_toolpath_g_opgolden` passes unchanged (as predicted).

## Gates

The shared tree held other agents' uncommitted session work (`op_draft.rs` failed clippy there), so the gates ran in a
clean worktree at `22e96b652` + W4, shared target dir; C4 (`e8072f172`) landed after and touches none of the W4 paths.
- fmt clean; `clippy -p rs_cam_core -p rs_cam_viz --all-targets --features heavy-tests,research,test-support -D
  warnings` clean.
- `cargo test -p rs_cam_core -q --lib`: 2727 passed. Viz `owner_tab_map_covers_every_id_f24`: passed.
- Green: the W4 sentry (4), G-OPGOLDEN, W4 conforms, h21, r3, the W1, W1b, W2 and W2b sentries, restlevels,
  `steep_shallow_cuts_only_the_rest_w6_4_6`.
