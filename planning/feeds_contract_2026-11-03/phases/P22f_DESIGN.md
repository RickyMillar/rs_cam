# P22f design: the geodesic slice cuts no full-slot perimeter sweep (2026-11-09)

Read-only design. HEAD `34c83e040` plus the uncommitted P22-0c-6a work of another agent (it does
not touch `adaptive3d/`). No file under `crates/` changed for this note. Evidence: one CLI run
(below) and the P22a readout (`phases/P22a.md`, commit `6fd852527`).

## 0. Summary

- On wanaka idx 1 ("Back bulk rough 18 mm", D 18) every region that cuts a perimeter sweep is
  refused by ONE check of `grown_outlines` (`adaptive3d/clearing.rs`): "inside margin under one
  planner cell". The check compares a fixed nominal margin, `SIMPLIFY_INSET + m` = 0.3 + 0.25 =
  0.55 mm, with the planner cell `max(R / 6, tolerance)` = 1.5 mm. All the other checks pass:
  the shortfall inset brings the measured margin to 1.510 mm (at least one cell), and the closing
  guard excess is 2 to 6 mm2 against an allowance of 500 to 1750 mm2.
- The check refuses every region of every Geodesic Adaptive op whose tool radius is over 3.3 mm
  (at tolerance up to R / 6). C31 wrote it before the shortfall inset existed (C22 §11.8: "a tool
  radius above 3.3 mm ... makes it fail. Such a region falls back, by design"). The shortfall
  inset added in §11.9 makes the measured margin at least one cell by construction, so the
  nominal check no longer guards anything that the measured check does not.
- Recommendation: **option (a)**, delete the nominal check (one arm of the decision chain and its
  doc line). The measured-margin check, the split check and the closing guard stay. Predicted on
  idx 1: the three sweeps go (6138 of the 8564 fed samples past `c D` plus both cells), and the
  opening power falls from 1.98 kW (ap 9, ae 17.8, F 8475) to at most 0.80 kW (ap 10, ae `c D`,
  F 8475) against 1.125 kW available. No pinned fixture moves: every pinned geodesic fixture has
  R at most 3.175 mm, so its cell is at most 0.53 mm and the check passes there today.
- Options (b) (plan the band where no grow exists) and (c) (cap the sweep's feed by `c / L`) stay
  for the regions that the split check or the closing guard still refuse. P22f-2 decides them
  after the P22f-1 readout shows whether any such region exists on wanaka.

## 1. Diagnosis

### 1.1 The capture

`grown_outlines` writes one `info` line, "C31 region opening", per region it judges. The P22a
test binary installs no tracing subscriber, and a crate edit was not allowed, so the capture used
the CLI, which installs an `EnvFilter` subscriber (`rs_cam_cli/src/main.rs`):

```
RUST_LOG="warn,rs_cam_core::adaptive3d::clearing=info" target/debug/rs_cam_cli project \
  planning/rivmap_block_job/nz_wanaka_21_operator.toml --output-dir <scratch>/p22f_cli \
  --skip 2,3,4,11,12,13,14,15,16,17,18,19,20,21,23,24 --no-adaptive-feed-modulation
```

The run generated toolpath ids 0 and 1 and simulated them (3.5 min, debug). It printed three
opening lines, one per region that reached the grow:

| Level z | Region | Cell (mm) | Nominal margin | Measured before | Shortfall inset | Measured after | Region area (mm2) | Closing excess / allowed (mm2) | Decision |
|---|---|---|---|---|---|---|---|---|---|
| 15.5 | 1 | 1.5 | 0.550 | 0.528 | 0.982 | 1.510 | 86851.1 | 1.96 / 1750.31 | inside margin under one planner cell: shipped sweep |
| 15.5 | 2 | 1.5 | 0.550 | 0.463 | 1.047 | 1.510 | 5695.9 | 6.22 / 500.85 | inside margin under one planner cell: shipped sweep |
| 11.0 | 1 | 1.5 | 0.550 | 0.463 | 1.047 | 1.510 | 5695.9 | 6.22 / 500.85 | inside margin under one planner cell: shipped sweep |

### 1.2 Each sweep to its region

The sweep cuts the region boundary inset by `R + PERIMETER_INSET_MARGIN_MM` = 9.25 mm.

- Z 16.50, moves 373-376 (P22a): the rectangle X 42.5..315.0, Y 119.5..399.5. Grown back by
  9.25 mm it is 291.0 x 298.5 = 86864 mm2, the area of region 1 at z 15.5 (86851.1 mm2) within
  the marching-squares staircase. The loop stands at Z 16.50, not 15.50: the lift
  (`lift_to_level`, `intended_floor::level_floor_z`) raises each point to the model floor where
  the model stands above the level.
- Z 15.50, moves 13582-13590: the loop X 95.0..196.5, Y 49.0..77.0. Grown by 9.25 mm it is
  120.0 x 46.5 = 5580 mm2, region 2 at z 15.5 (5695.9 mm2; the rounded corners and the staircase
  give the difference).
- Z 11.00, moves 17797-17805: the same loop one level down, region 1 at z 11.0 (the same
  5695.9 mm2).

### 1.3 Why the nominal check refuses, and why it is redundant

The decision chain (`grown_outlines`, in order): (1) no smoothed or grown outline; (2) the
shortfall inset split the outline; (3) the nominal margin under one cell; (4) the measured margin
after the inset under one cell; (5) the closing guard; else grown.

- (3) reads `SIMPLIFY_INSET + m < cell`. Both terms on the left are fixed constants (0.3 and
  0.25 mm); the cell grows with R. At R 9 the cell is 1.5, so (3) refuses every region, whatever
  the region's real margin. The threshold is `R = 6 (SIMPLIFY_INSET + m)` = 3.3 mm.
- (4) reads the real margin of the outline the plan uses. C22 §11.9 proves the shortfall inset
  gives `measured_after >= measured_before + k = cell` (a point of the inset by `k` has a disc of
  `k + d` inside the raw region), with the flattening budget added. The run confirms it:
  1.510 >= 1.5 on all three regions.
- So (3) protects nothing that (4) does not. The reason C31 gave for (3), "the reach of an out
  cell's nearest read plus the boundary error, about one cell" (§11.8), is exactly what (4)
  measures on the inset outline. (3) dates from before the inset (§11.8, then the inset in §11.9
  "before D19b"); after the inset it reads the margin of an outline that the plan does not use.

### 1.4 The silent paths

After `grown_outlines`, `clip_to_stock_box` (C38 item B) and `clip_to_centre_reach` (C38 §13.7)
return `None` with no log line when nothing is left. On the three wanaka regions they did not
run their `None` path, because the grow already refused. After option (a) they run: region 1
(X 33.25..324.25 grown) lies inside the stock box (X -18.5..376.5, Y -8.5..451.5), and the
centre reach is the op's own containment grown by `R - m`, so both are expected to return
`Some`. This is not measured. P22f-1 must add the clip result to the same `info` line, so a
silent `None` is seen.

### 1.5 The fixture (shelf pocket, D65 sentry fixture)

Not captured: the test binary has no subscriber, and the fixture lives in test code, not in a
project file the CLI reads. Derived instead: R 3, tolerance 0.1, so the cell is
`max(3 / 6, 0.1)` = 0.5 mm, and (3) reads 0.55 >= 0.5, which passes; (4) holds by construction.
The P22a fixture arm agrees: no sample falls in the perimeter class there. So the shelf regions
are expected to read "grown, no sweep".

## 2. Options

### (a) Delete the nominal check

- Mechanics: remove the `nominal_margin < cell` arm from the decision chain and its bullet from
  the doc of `grown_outlines`. Keep `nominal_margin_mm` in the `info` line as a reading only, or
  drop it with the arm. (1), (2), (4) and (5) stay unchanged.
- Why the guard that refuses today is not right: it judges a constant against a cell that grows
  with R, so for a large tool it refuses even when the real margin (4) is met. Its reason is
  covered by (4) on the outline the plan uses (1.3).
- What it does to idx 1: the three regions read "grown, no sweep" (measured after 1.510, closing
  excess well under the allowance), subject to the two clips (1.4). The geodesic field and the
  2D cleanup then open the band under the 2D pass load, as C31 designed for the research arm.
  The 2D tool-centre region stands 1.51 mm inside the raw region, so at a wall the waterline
  meets up to `1.51 + cell / 2` = 2.26 mm, 0.126 D, under `c` 0.333.
- Risk: a large tool on a staircase region now opens on a grown outline whose shortfall inset is
  about a whole cell (0.98 to 1.05 mm here). That is the case §11.9 designed the inset for; no
  new risk beyond C31's own, which the C12 judge measured at R 3.

### (b) Where no grow exists, plan the band under the pass load

For a region that (2) or (5) still refuses:

- b1: the geodesic field on the raw region (no smoothing, no grow), so the 2D tool-centre region
  is the region inset by R, and a second, outer ring at the sweep locus cut AFTER the body from
  inside. C22 §11.3 candidate 2 measured that ring on Corne: median gap 0.2 to 0.5 mm, p95 at
  the ceiling, corners up to `R + 2.5` mm over it. It needs inner rings or a 3D load read at the
  corners.
- b2: a 2D pass ladder from the inside out: offset rings of the region inset by R, each step at
  most the spacing law `spacing_bound(R, c D, rho)` (`geodesic/wraps.rs`), replayed under
  `step_within_pass_load`. This is the corridor problem of P22e; it reuses the same law.
- What it does to idx 1: nothing after (a), unless a region falls to (2) or (5). Today none does.

### (c) Cap the sweep's feed by `c / L`, as D65 does

- Mechanics: give the sweep's Cut a `FeedCapStart { factor: c / L }` with `L` its read load
  (`step_load` on the planner stock); for a fresh-stock sweep `L` = 1 (a full slot), so the
  factor is `c` = 0.333 and the feed 2821 mm/min.
- Power at the sweep after (c): ap 9: 1.10 kW; ap 10: 1.22 kW (formula in 4), so (c) alone does
  not hold 1.125 kW at the full Depth/Pass. The P20g modulator lowers it further after the
  simulation.
- Why the planner should not rely on it: the 2D rule bounds the width at `c D`, not the power;
  a full slot of an 18 mm tool at ap 10 is a deflection and chip-evacuation load that a lower
  feed only reduces. A feed cap hides a width the planner chose; option (a) removes the width.
  (c) stays useful as the net for a region that (b) cannot plan, with a finding, per the "never
  refuse a job" ruling.

## 3. The reasons the design keeps (C31, C38)

- C31 (C22 §11.3, 11.7-11.9): the grow must keep each planned centre at least one planner cell
  inside the raw region, because the region boundary is the tool-centre limit only to half a
  cell and the lift reads the nearest heightmap cell (another half cell). Check (4) holds this
  on the inset outline; (a) keeps it.
- C31 closing guard (§11.8 item 4): growing by `R - m` and insetting by R closes gaps narrower
  than about 2R; the guard limits the closed area to one cell times the boundary length. (a)
  keeps it.
- C38 item B (`P3_C38_outer_band.md` §5): the grow stops at the stock box, where no material
  stands beyond the edge (a phantom band otherwise). (a) keeps the clip.
- C38 §13.7: the grow stops at the centre clip's reach (containment grown by `R - m`), so nothing
  is planned where the boundary clip would drop it (C22's "cut, unfed" turns). (a) keeps it.

## 4. Recommendation and predictions

**Option (a) now (P22f-1); then (b) and (c) only if the P22f-1 readout shows a region that (2)
or (5) refuses (P22f-2).**

Power law (the gate's, through `PowerTerms`; the true-width constants from P20i, `Ks` 58.516
N/mm2, `F_edge` 4.595 N/mm, which reproduce P20i's 0.985 kW at ap 10, ae 8.8, F 6999):

```
P = Ks ap ae F / 6e7  +  F_edge ap (pi D n) (z psi / 2 pi) / 6e7,   psi = acos(1 - 2 ae / D)
D 18, n 18000, z 2
```

| Engagement | Power |
|---|---|
| Sweep today: ap 9, ae 17.8, F 8475 | 1.98 kW |
| Sweep today at the full Depth/Pass: ap 10, ae 17.8, F 8475 | 2.20 kW |
| Opening after (a): ap 10, ae `c D` 5.99, F 8475 | 0.80 kW |
| Feed that holds 1.125 kW at ap 9, ae 17.8 | 3012 mm/min |

The available power is 1.125 kW (P20). After (a) the opening is under it at the op feed.

Fed samples past `c D` on idx 1 (P22a classes):

| Count | Today | After (a), predicted |
|---|---|---|
| Past `c D` | 10921 | about 4759 (today less the 6162 sweep samples), plus the band's own departure windows and capped steps (expected small; the band is planned under the 2D rule) |
| Past `c D` plus both cells | 8564 | about 2426 plus the same small term; the ramp entries (2066) then lead, which is P22b's target |

## 5. Pins at risk and the new sentry

| Pin | Tool | Cell | Moves under (a)? |
|---|---|---|---|
| `geodesic_adaptive_keeps_the_research_geometry_s7_4` | R 3, tol 0.1 | 0.5 | no: (3) passes today |
| `the_geodesic_preset_keeps_its_plan_s7_3` | 2D preset (no 3D grow) | - | no |
| `adaptive3d_plans_every_slice_piece_g_adapt3dpieces` | D 6 | 0.5 | no |
| G-OPGOLDEN `adaptive3d/*` rows | D 6 (`op_fixtures.rs`) | 0.5 | no; the 46 known rows stay, no new row |
| C12 judge `geodesic_adaptive_holds_the_pass_load_c12` | R 3 | 0.5 | no |
| `a_geodesic_step_over_the_ceiling_is_cut_capped_d65`, `a_geodesic_cap_survives_every_dressup_s7_4` | R 3 | 0.5 | no |
| P22a wanaka arm | D 18 | 1.5 | yes: the perimeter class goes to 0 (the intended move) |

Prediction: (a) is byte-identical on every fixture with `max(R / 6, tolerance) <= 0.55`.

New sentry `a_geodesic_slice_cuts_no_full_slot_sweep_p22f`: the D65 shelf fixture scaled by
`D / 6` (every length of `a_geodesic_step_over_the_ceiling_is_cut_capped_d65` times 3, with an
18 mm tool, so the cell is 1.5 and the geometry is the repo's own). Claims:

1. No uncapped clearing cut fills a whole entry-to-retract run as a loop that closes on its start
   (the P22a perimeter rule, reused from `every_wide_step_has_a_named_source_p22a.rs`).
2. No fed sample of an uncapped flat clearing move reads over `c D` plus both cells with a
   removal ratio at or above the named classes' (the arc-only thin rings stay allowed).

Red at HEAD (the scaled regions refuse at (3)); green after (a). Inject check: put (3) back and
the test goes red.

## 6. Phase split and blast radius

- **P22f-1**: delete (3) in `grown_outlines`; add the two clip results to the "C31 region
  opening" line (so a silent `None` is seen); the new sentry; rerun the P22a arms (fixture and
  wanaka) and G-OPGOLDEN. One implementer, one file under `src/`
  (`adaptive3d/clearing.rs`), plus the folder doc (`adaptive3d/CLAUDE.md`, the sentry name).
- **P22f-2** (only if needed): (b) and (c) for regions that (2) or (5) refuse.

`rg` at this tree:

| Item | Count |
|---|---|
| `nominal_margin` in `src/` | 3 lines, all in `grown_outlines` |
| "inside margin under one planner cell" in `src/` and `tests/` | 1 (the decision string) |
| `PERIMETER_INSET_MARGIN_MM` in `src/` | 7 lines (unchanged by (a)) |
| Core test files that name Geodesic Adaptive | 42; the only ones with a tool over 6.6 mm are the wanaka project loaders (evidence, `#[ignore]`); `param_sweep.rs` sweeps the 3D strategies with a 6.35 mm tool (cell 0.529, under 0.55: no move) |

## 7. Open questions

1. Delete (3), or keep it as a reading in the log? Recommendation: delete the arm, keep
   `nominal_margin_mm` out of the line too (it no longer describes the outline the plan uses).
2. Should a region refused by the closing guard or the split still cut a full-slot sweep?
   Recommendation: no; P22f-2 replaces the sweep there by (b2), and (c) with a finding where (b2)
   cannot plan, per "never refuse a job".
3. The two clips return `None` silently. Recommendation: log their result in the same line in
   P22f-1 (no behaviour change).
4. The shortfall inset at R 9 is about one cell (1 mm). The 2D centres then stand 1.51 mm inside
   the raw region, so the waterline at a wall meets up to 2.26 mm (0.126 D). Recommendation:
   accept (under `c` 0.333); read the waterline samples in the P22f-1 wanaka arm.
5. The P22a test cannot capture tracing (no subscriber). Recommendation: when P22a-2 resumes,
   add the opening decision to the semantic trace or `Produced` (one record per region), so a
   test reads it without a subscriber, as the removed C31 research record did.
6. Ordering against P22b: P22f-1 first (it removes 72 % of the wide samples on idx 1), then P22b
   (the ramp entries lead after it). Recommendation: keep the order note 10 of `P22_DESIGN.md`
   states.

## 8. Lead rulings (2026-11-09 night)

1. Delete arm (3), the nominal-margin check, and drop `nominal_margin_mm` from the log line: since
   the §11.9 shortfall inset the measured check guarantees one cell by construction, so (3) guards
   nothing and refuses every geodesic op with R > 3.3 mm.
2. No region cuts a full-slot sweep after P22f: where the split (2) or the closing guard (5)
   refuses, P22f-2 plans the band (b2), and where no plan exists caps the sweep at `c / L` with a
   finding that names the toolpath and the region (never refuse a job). P22f-2 runs only if the
   P22f-1 readout shows such a region on the fixtures or on wanaka.
3. Log the two clip results in the "C31 region opening" line (P22f-1, no behaviour change).
4. Accepted: the 2D centres stand about one cell inside the raw region (0.126 D at the wall on an
   18 mm tool, under `c`); the P22f-1 wanaka arm reads the waterline samples and states them.
5. P22f-1 itself records the opening decision per region (the case, the margins, the two clip
   results) in `Produced` or the semantic trace, so the new sentry and the P22a arms read it
   without a tracing subscriber. P22a-2 does not grow for it.
6. Order: P22a-2 (in the tree now), then P22f-1, then P22-0c-6b, then P22b. Note 10 of
   `P22_DESIGN.md` stands.

Gates of P22f-1 (lead): the new sentry `a_geodesic_slice_cuts_no_full_slot_sweep_p22f` red at
HEAD and green after; the P22a fixture and wanaka arms (sweep class 0 on wanaka idx 1; the
histogram and the power at the opening in the note); G-OPGOLDEN the 46 known rows; s7_4, s7_3,
adapt3dpieces, D65, planner_never_ahead, emission_byte_parity; the C12 judge
(`geodesic_adaptive_holds_the_pass_load_c12`, heavy, about 9 min: run it once, at night);
core `--lib adaptive`; fmt; clippy. A wanaka G-code change on idx 1 is expected and is the
point; list the motion-line and F-word counts before and after (release CLI, plain).
