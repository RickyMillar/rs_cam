# C3, the D55 port (G-AIRPECK): the plunge entry pecks from the contact read

Date: 2026-10-28. Source: research commit `f2724a5bd`. Built and measured
in an isolated detached worktree at `e5614d2b8` (C2), with its own lane
script and target directory. Gates repeated on `b8aee0bf2` (the shared
HEAD at the freeze) plus the patch. Both worktrees and the target are
deleted. The implementer did not commit.

Patch: `c3_d55_tree_vs_e5614d2b8.patch`, md5
`1d9953b17d4bd996fb9f992f99d9b5a6`. 10 files (9 changed, 1 new).
`git apply --check` passes on `b8aee0bf2`. Pre-registration:
`c3_d55_PREREG.md` (md5 `d01f03fde2ec0fc51581da8345837ef0`), written
before the change.

## 1. Change

- `adaptive3d/path.rs::emit_peck_plunge` takes `contact_top`
  (`RapidWithFloor::contact_z`, which the helix and the ramp already take).
  The rapid still stops at the sliver-safe floor. The ladder starts at
  `min(ladder_start, max(contact, entry.z) + entry_clearance_mm)`, and one
  straight feed goes through the air above it. Master's names are the same
  as the research base: no renames.
- Lead ruling (option 2): the air feed runs only when the air gap is more
  than `MIN_VERTICAL_MOVE_MM` (0.1). This is the function's existing 0.1 mm
  rapid-skip number (G-PECKROOT), now named once and used by both rules.
  Below it, the ladder starts at the old start. This removes the two
  0.0000176 mm feeds that the plain research port added. No new number.
- The emitter tests `a_plunge_through_cleared_air_is_one_stroke_d55`,
  `a_plunge_into_real_stock_still_pecks_d55`,
  `air_above_real_stock_feeds_then_pecks_d55` (`--lib _d55`), and the
  session sentry `a_rest_entry_does_not_peck_through_cleared_air_d55`, are
  ported as they are.
- `adaptive3d/CLAUDE.md` names the sentries (40 lines).

## 2. Re-bless list (old → new, first differing move)

Every XY move is identical bit for bit in every item. Only vertical moves
change (`EntryPlunge` feeds and `Retract` rapids).

| Item | Moves | XY moves | Feeds, retracts removed | First differing move |
|---|---|---|---|---|
| parity `contour_parallel` | 433 → 421 | 329, identical | 6, 6 | 130: plunge to Z 6.5 → Z 6.0 |
| parity `contour_parallel_flat_blend` | 1198 → 1197 | 957, identical | −1, 2 (one feed added by design, below) | 327 |
| parity `contour_parallel_by_area` | 393 → 382 | 295, identical | 5, 6 (one feed added by design) | 130 |
| parity `geodesic_adaptive` | 1829 → 1432 | 1073, identical | 191, 206 | 298 |
| parity `geodesic_adaptive_boundary` | 901 → 839 | 710, identical | 28, 34 | 261 |
| op golden `adaptive3d/fresh` | 379 → 368 | — | rapids 18 → 12, linear 361 → 356 | cutting length 1958.852 → 1955.852, rapid length 105.62 → 102.62, spans 86 → 80, both hashes |
| **NOT pre-registered:** `geodesic_adaptive_keeps_the_research_geometry_s7_4`, "island pocket, two levels" | 1976 → 1941 | 1807, identical | 16, 19 | 770 |

- The op golden's other 3D Rough rows (`rest`, `by_area_helix_cp`,
  `ramp_geodesic`) and every other row do not move. This is as predicted.
- The S7.4 pin is a geometry pin of the Geodesic plan, vertical moves
  included. I missed it in the pre-registration. Its other three fixtures
  do not move. The pin is updated and its doc names C3. A probe (deleted)
  dumped the moves at HEAD and with the port to give the XY check.

**Feeds added, by design** (the designed D55 air feed; the retracts stay,
and every stroke is at most one Depth/Pass):

| Case | Site Z | Old | New |
|---|---|---|---|
| `contour_parallel_flat_blend` | rapid floor 10.131 | 10.131 → 7.131, retract, → 4.766 | air 10.131 → 9.002, 9.002 → 6.002, retract, → 4.766 |
| `contour_parallel_by_area` | rapid floor 5.954 | 5.954 → 2.954, retract, → 1.000 | air 5.954 → 5.319, 5.319 → 2.319, retract, → 1.000 |

Option 2 changed only the two micro-feed sites (`contour_parallel` 422 →
421, `flat_blend` 1198 → 1197, against the plain port). Every other count
is the same as the plain port's. (My message to the lead said 3 micro-feed
sites; there were 2.)

## 3. Gates (isolated tree)

| Gate | Result |
|---|---|
| `a_rest_entry_does_not_peck_through_cleared_air_d55`, `--lib _d55` | 1/1, 3/3 |
| byte parity, op golden, S7.4 (after the re-bless) | 6/6, 1/1, 1/1 |
| the same four on `b8aee0bf2` + the patch | all ok |
| `rs_cam_core --lib` | 2723 ok |
| the 46 light core binaries that build a 3D Rough | all ok except the S11 `NotSet` base reds: `adaptive3d_lift_bridge_b1`, `identity_setup_emission_frame_audit` (2), `safe_z_emission_frame_g_safez_local` (2), all "feeds not set" before any motion |
| `rs_cam_cli` (every test) | all ok |
| clippy `rs_cam_core --all-targets`, full features | clean |
| `cargo fmt --all -- --check` | clean |

Not run: the 3 heavy binaries that build a 3D Rough
(`adaptive3d_interior_cell_parity_f029`, `adaptive3d_planner_stock_xy_f027`,
`strategy_advisor_smoke`), and the core full gate.
