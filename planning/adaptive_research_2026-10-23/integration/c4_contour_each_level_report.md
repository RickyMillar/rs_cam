# C4, "Contour each level" (G-RESTDOC; operator ruling D69, 2026-10-28)

Date: 2026-10-28. Built in an isolated worktree (own lane script, own
target directory), rebased onto `geodesic-integration` `2833d66bc` (W2b).
The patch applies to the shared HEAD `22e96b652`; nothing in core changed
after `2833d66bc`. The implementer did not commit.

Patch: `c4_contour_each_level_tree_vs_2833d66bc.patch`, md5
`0845d290e2d5e4d3808c31453eb555ad`. 37 files (34 changed, 3 new), +1482/−29.
Pre-registration: `c4_PREREG.md` (md5 `b5f446462d3a566375857b0021f9cdc5`),
approved with the load-cap addition.

## 1. What it does

- `Adaptive3dConfig::contour_each_level: bool`, off by default; with it
  off, no key is saved (`skip_serializing_if = "std::ops::Not::not"`).
  `#[param(help)]` puts it in the operation schema, so MCP
  `get_operation_schema` and `set_toolpath_param` see it. GUI label for
  I4: "Contour each level".
- Planner: `Adaptive3dLinking::level_contour: Option<LevelContour {
  link_kinematics }>`, filled by `compute/execute/rough_3d.rs` from the
  config and `ExecutionContext::link_kinematics`.
- With the option on, on remaining stock with a centre containment:
  1. Anchor (D63 port, `805368f98`): `PriorStockTops`, `tool_reach_cells`,
     `level_anchor_z_in_reach`, `rest_ceiling_keeps_doc_in_reach`. Off:
     today's functions run unchanged.
  2. New file `adaptive3d/level_contour.rs`: the band (reach cells
     outside the containment, with material before the boundary clear);
     after each `clear_planned_level` (Global and By Area), one contour
     along the containment where the band stands. The line is the ring
     moved `FlattenPolicy::UNTOLERANCED_MM` inside. Z is max(floor, top in
     reach − DPP − tolerance), where "top" reads the band AND the planner
     stock above its floor. A point stands when the band top in reach is
     more than one planner cell above Z. Entries and cuts go through
     `push_segment_with_stamp` (`plan_entry`, stay-down).
  3. Merge: `machine::kinematics::retract_saves_time` with the machine
     kinematics; no kinematics, no merge; never across rings.
  4. Load cap (the lead's addition): each run is read by the one load
     (`step_load_on`, a new reader form of `step_load` that sees the
     planner stock plus the band; `step_load` calls it unchanged). A run
     over the Geodesic step ceiling
     (`radial_woc_fraction_from_leading_arc(pass_engagement_limit)`) is
     cut between `FeedCapStart`/`FeedCapEnd` at the lowest
     `capped_step_factor`, so its moves carry `Move::feed_cap`. The level
     span gets counters `contour_runs`, `contour_peak_load` and
     `contour_min_cap_factor`.
- On a fresh rough the option changes nothing (sentry).

## 2. Defects found and fixed while writing C4

1. **Nondeterminism.** The containment polygon's first vertex varies from
   run to run in one process. A closed contour started there, so three
   identical runs gave 117, 118 and 126 moves. Fix: a closed run starts at
   its point nearest the tool (ties X, then Y). The ring is simplified from
   its least-X, least-Y vertex (RDP on a closed ring depends on its start).
   Sentry `the_option_is_deterministic_c4`. The varying first vertex
   upstream is pre-existing; option-off output does not depend on it.
2. **The band state was not the shipped geometry** (the residual of the
   first report). The band was read along the ring and lowered along the
   cut that the emitter ships, which is drape + RDP + blend (now one shared
   `clearing::shipped_cut_path`, which `stamp_emitted_segment` calls with
   the same arithmetic). The RDP chord left band cells in read reach that
   no shipped cut lowered; they held the contour at Z 6.9 on every level,
   and the level clearing at Z 1 then cut 5.9 mm deep. Fix: the ring is
   simplified once at the op's chord tolerance before the Z rule, so the
   line that is read is the line that ships.

## 3. The lead's cell-size check (D63 fixture, DPP 3, option on)

Before fix 2 (first report): 0.5 → max 3.750; 0.25 → 5.900; 0.125 → 5.900
(over volume 6.2 %, 7.4 %, 18.1 %): a real over-cut that the 0.5 mm cell
under-read, not a sim artefact. After fix 2:

| contour each level | sim cell | max axial | over DPP + cell (mm³ of removed) | samples |
|---|---|---|---|---|
| off | 0.5 | 9.000 | 4327 of 7504 | 479 |
| off | 0.25 | 9.000 | 4457 of 7622 | 959 |
| off | 0.125 | 9.000 | 4613 of 7782 | 959 |
| on | 0.5 | 3.000 | 0 of 10136 | 0 |
| on | 0.25 | 3.000 | 0 of 10278 | 0 |
| on | 0.125 | 3.000 | 0 of 10186 | 0 |

The strict rule (max ≤ DPP + one sim cell) holds; no new bound or margin.

Load on sentry 3: peak 0.75 before the cap against the ceiling 0.3626;
lowest cap factor 0.4835, so the peak load per unit of feed is 0.3626 (the
ceiling); 39 capped moves.

## 4. Corne play job (operator-approved release run; `--setup 0`)

`play/corne_geodesic_play.toml` (off) and a new copy
`play/corne_geodesic_play_c4on.toml` (the key on the three 3D Roughs).
Acceleration clock (`phase3/d69/opsum.py`), over DPP
(`phase3/d69/depth_by_level.py`, axial > DPP + 0.05).

| | edge op s | entries | max axial | edge op over DPP | over DPP+3 | copy op s | job s |
|---|---|---|---|---|---|---|---|
| C4 off | 471.1 | 97 | 8.96 | 6750 mm³ (12.6 %) | 0 | 1687.3 | 3754.7 |
| C4 on | 842.3 | 320 | 12.0 | 3138 mm³ (5.6 %) | 10 mm³ (2 samples) | 2031.0 | 4466.7 (+712, +19 %) |
| D68 today | 453.7 | 132 | 8.96 | 6750 (12.6 %) | 0 | — | 3034 (D68 job) |
| D68 option 2 | 767.6 | 293 | 12.0 | 2982 (5.3 %) | 268 | — | 3702 |
| D69 option 2 + merge | 745.2 | 312 | 12.0 | 3170 (5.6 %) | — | — | 4973.1 (D69 job, +15 %) |

- The off run equals D68's baseline over-DPP to the mm³. The times differ
  from D68 because the code moved since (C2, D55 and more).
- Over DPP+3 falls to 10 mm³, from 268 in the prototype. The 12.0 mm max
  stays (2 samples at Z about 5). Over-DPP 5.6 % is the 0.05–3 mm class
  D68 called the drape and tolerance class (G-AXIALDOC).
- The copy op costs +344 s (D68 saw +354 s): op 5 leaves less stock, and
  its first level moves.
- Cost: +19 % job time on Corne (D69 estimated +15 to 21 %).

## 5. Gates (rebased tree)

| Gate | Result |
|---|---|
| `a_rest_op_holds_one_depth_at_the_band_c4` | 5/5 (anchor, holds one DPP, load cap, off pin, determinism) |
| `contour_each_level_is_off_by_default_c4` | 2/2 (default and serde and schema and setter; fresh rough unchanged) |
| `--lib _c4` (merge) | 1/1 |
| planted reds | pass off: "holds" fails (9.0) and "caps" fails (no cut); merge off: "the 1.5 mm gap must be fed through" |
| byte parity, opgolden, s7_4, d50, d55 | 6/6, 1/1, 1/1, 1/1, 1/1 (option off: unchanged) |
| `rs_cam_core --lib` | 2728 ok |
| `rs_cam_cli` (every test) | all ok |
| clippy `rs_cam_core --all-targets`, full features; viz and mcp check | clean |
| `cargo fmt --all -- --check` | clean |

Not run: the core full gate; the light 3D Rough batch on the rebased tree
(the option-off paths are unchanged by construction, and parity is
byte-identical).

## 6. Re-bless list

None. Mechanical literal edits: 21 test or src files gain
`level_contour: None`, 7 gain `contour_each_level: false`. Docs:
`adaptive3d/CLAUDE.md` (file map, sentries; 40 lines), `FEATURE_CATALOG.md`
(the 3D Adaptive row).

## 7. Queued

- C4b (lead): By Area runs the wall cleanup per region-level with a cell
  mask when the option is on (the F9 fix). Not started.
- GUI checkbox (I4): label "Contour each level".
