# P3 C38 — the Corne outer band: cause, research-arm design, shipped fix

Agent C38 (the C12 agent), 2026-10-26. Design note and run plan only: **no
code is changed.** File:line citations are at `b37cad1a4` (the D20 build;
`.claude/worktrees/adaptive-head` HEAD; C22 edits that tree now, so read
the lines with `git show b37cad1a4:<path>`). Sources: R4 §1.5 and §7.

C38 covers two items (lead, 2026-10-26):

- **Item A, the F-038 skip** of the outer strip below Z 17 (§1–§4).
- **Item B, the C31 grow at edges that are not walls** (§5): the
  outer-strip descents at Z 17 (R4 §7.2: 124).

§6 gives the line between C38 and C22's C37.

## 1. The cause: confirmed from the D20 artefact, one run pins the rest

### 1.1 Already proven by the existing D20 output (no run needed)

`phase3/fix7/corne/geodesic_local_zigzag_pick/tp_1_3D_Rough_1.json` (the
debug trace of the D20 run, `run_fix7.sh`, b37cad1a4) holds one
`agent2d_region` span per region. Read now:

| Span | Counters |
|---|---|
| `region 1/1 @ Z 17.000` | none (planned) |
| `region 1/2 @ Z 11.000` | `forecast_cut_length_mm 0.0`, `skipped_f038 1.0` |
| `region 2/2 @ Z 11.000` | none (planned) |
| `region 1/2 @ Z 5.000` | `forecast_cut_length_mm 0.0`, `skipped_f038 1.0` |
| `region 2/2 @ Z 5.000` | none (planned) |
| `region 1/2 @ Z 0.500` | `forecast_cut_length_mm 0.0`, `skipped_f038 1.0` |
| `region 2/2 @ Z 0.500` | none (planned) |

`phase3/fix7/corne_openings.txt` has a `C31 region opening` line for
region 1 at Z 17 and for region 2 at Z 11, 5 and 0.5, and none for region
1 below Z 17. The C31 call (`clearing.rs:2944`) comes after the gate
(`clearing.rs:2844-2910`), so a skipped region never gets an opening line.
So below Z 17 region 1 of each level was skipped with a forecast of 0 mm
against `min_region_cut_length_mm` 15. C22 §10d read the same spans on D18.

The one gap: the spans do not say WHERE region 1 is. The debug line of the
skip (`clearing.rs:2896-2904`) gives its area. The run below gives it.

### 1.2 The command for D9

Run it in the fixture worktree, whose HEAD is `b37cad1a4` and is clean (the
D20 build). Do not run it in `adaptive-head`, because C22's edits there are
in flight. Leave out `--sim-artifact`, because the cause needs only stderr
and the trace (the artefact is 388 MB).

```sh
R=/home/ricky/personal_repos/rs_cam
cd $R/.claude/worktrees/fixture-cdc9814e
git log --oneline -1          # expect b37cad1a4
D=$R/planning/adaptive_research_2026-10-23/phase3/c38/corne_d20_debug
mkdir -p $D
RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug \
RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_zigzag_pick \
timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli \
  --features rs_cam_core/research -- project \
  $R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml \
  --output-dir $D --setup 0 --skip 4 --summary --accept-unmodeled-tool-load \
  > $D/stdout.txt 2> $D/stderr.txt
echo rc=$?
grep -E "F-038: skipping micro-region|C31 region opening" $D/stderr.txt
```

The filter turns on `debug` for `adaptive3d/clearing.rs` only. A crate-wide
`debug` on Corne writes a very large log.

**The lines that prove the skip** (P = predicted):

- P1.1: three lines `F-038: skipping micro-region (forecast cut length
  below threshold)`, each with `region=1 region_total=2 forecast_mm=0
  threshold_mm=15`, at `z=11`, `z=5` and `z=0.5`. These are the only skips
  of a whole region.
- P1.2: their `area_mm2` is that of a strip 3 mm wide (wall + R to the
  containment line, in tool-centre space) along the outer wall: about
  3 mm × the strip length. I cannot derive the length from the files I
  read, because the stock box can cut the strip short. The value to read
  is `area_mm2 / 3`, a length in mm. It must be of the order of the case
  outline (some hundreds of mm). If `area_mm2` is that of a large pocket,
  region 1 is not the strip and the cause chain is wrong.
- P1.3: the `C31 region opening` lines are the four of
  `fix7/corne_openings.txt`: region 1 at Z 17, region 2 below. None is for
  region 1 below Z 17.
- `tp_1_3D_Rough_1.json` in `$D` repeats the counters of §1.1.
- The other debug lines of this filter, `F-038: dropped short cut groups
  inside 2D adaptive output` from `plan_one_outline` (`clearing.rs:2438`
  block), are the group filter after a plan. They are not the region gate.
  Count them, but they do not prove the skip.

Optional location check: region 1 owns the first waterline contour of each
level below Z 17 (C22 §11.2: it starts at (20.3, -106.2)). If D9 also runs
with `--sim-artifact`, the samples of that waterline must lie 3 to 9 mm
outside the case outline.

## 2. What the research arm does in the strip once the gate lets it through

I read the code at b37cad1a4 to see whether a fix of the gate alone cuts
the strip. **It does not.** These are predictions from the code; no run
measured them.

1. **C31 opens the strip on a corridor about 2 mm wide.** The strip is
   3 mm wide in tool-centre space. `research_grown_outlines`
   (`clearing.rs:2634`) smooths it (inset 0.3, RDP 0.3), insets it again by
   the shortfall until the measured margin is at least one planner cell
   (0.51 mm on Corne region 2), and grows it by `R - m`. The 2D
   tool-centre region is the raw region inset by about 0.51 mm on each
   side: a corridor about 3 − 1.02 ≈ 2.0 mm wide. In material space the
   grown outline is about 2.0 + 2R ≈ 8.0 mm wide.
   - Risk: where the strip tapers (at the stock box, at a corner), the
     shortfall inset can split it. The decision is then "the shortfall
     inset split the outline: shipped sweep" (`clearing.rs:2708`), and the
     shipped path gives nothing in a 3 mm strip: the sweep's inset by
     `R + 0.25` is empty, and `planner_outlines` insets the smoothed strip
     by R again.
2. **The field does not apply.** `plan_piece` (`research_geodesic.rs:4817`)
   needs the starter pocket. `emit_helical_starter_pocket` needs a material
   clearance of `2R` (`adaptive/path.rs:2671`), which is a centre clearance
   of R. The corridor half-width is about 1.0 mm, so it returns `None`, and
   the slice falls back to the agent on the SAME grown piece. This answers
   the brief's question. In a corridor narrower than `2R + 2s` in material
   space (8.4 mm on Corne), the seed rim of radius R does not fit and the
   kernel gives `None`. The kernel does not need a corridor to be wider
   than `2R + 2s`; it needs a centre clearance of R for the seed.
3. **The fallback insets once, not twice, on the grown piece, and still
   cuts nothing.** The agent's narrow gate (`is_narrow_machinable`,
   `adaptive/path.rs:2790`) finds the corridor narrower than `R + s` and
   plans contour loops (`contour_parallel_segments`). Under
   `WithinPassLoad` a loop is cut only where each step holds the pass load
   (`adaptive/path.rs:2884`). In a fresh strip every step is a slot: the
   disc (6 mm) stands inside an 8 mm strip of material, so the swept width
   is about D, against the ceiling `c = 0.2189` (R4 §2.1). Every span is
   refused and left for the mop, and the mop holds the same load. So the
   plan is empty, or nearly empty.
   - R4 says "the agent fallback insets again and gives nothing". The
     outcome is right, but the reason is the pass load, not a second
     inset.
4. **The strip is closed on both sides.** One side is the case wall. On
   the other side, real stock stands out to 9 mm from the wall and beyond,
   and no column cuts beyond 9 mm (R4 §1.5, the 2423 mm² band). The
   planner's own stock reads air beyond the containment line, because the
   boundary clear empties those cells (`adaptive3d/path.rs:835-868`, O5b),
   but the real stock is there (§5.1). So the first pass along the strip is
   a slot whatever the producer. No producer holds `c` there. This is
   geometry, not a defect of the field.
5. The 2D grid reads the grown outline as the material edge. The cutter
   disc reaches at most `containment + R − 0.51` = 8.49 mm from the wall,
   so it never meets the out-of-scope stock beyond 9 mm. There is no hidden
   under-read there.

**Consequence:** each option below that only changes the gate gives the
same band as D20, because the planner has nothing to emit. The fix needs a
gate change AND a corridor opening that a slot can carry.

## 3. The design (research arm only)

### 3.1 The options for the gate

| Option | What it does | Strip result (P) | Verdict |
|---|---|---|---|
| G-a | Forecast on the outline the planner plans (C31 grown pieces): perimeter of `grown inset (R + m)`, then a dry run of the producer on the grown piece | Perimeter estimate: a 1.5 mm corridor loop, about 2 × L ≫ 15 → passes. Then the plan is empty (§2.3) → nothing cut | Necessary, not sufficient |
| G-b | No gate for the research arm: plan the region, then measure the plan's own cut length. Skip the region only when the plan cuts less than the threshold | Same as G-a for the strip. One plan instead of a dry run plus a plan | **Recommended**: the plan is the forecast, at no extra cost |
| G-c | Forecast on the field's wrap length | `None` in a corridor → undefined | Rejected |

G-b uses the existing F-038 group filter (`plan_one_outline`) for the
"micro-region" judgement. A region whose plan cuts less than
`min_region_cut_length_mm` keeps only its first group, as now. A plan of
length 0 emits nothing. So a micro-region is "a plan that cuts almost
nothing", not "a forecast that saw nothing". No new constant.

### 3.2 The corridor opening (needed for any strip to be cut)

When the field gives `None` because the seed does not fit, AND the 2D
tool-centre region of the piece has a largest inscribed radius `ρ_max < R`
(a corridor), plan the corridor as follows:

1. **A medial slot.** One pass along the corridor's medial line: the
   tool-centre region inset by `ρ_max − cell`, walked as one loop or one
   open path per piece. In material space the slot clears the medial line
   ± R.
2. **Side passes that hold the load.** Offsets of the medial path at
   `k · s` (k = 1, 2, …) outward on each side, clipped to the corridor,
   until the corridor edge. Each pass bites at most `s` sideways next to
   the slot, so it holds the pass load like any ring pass. On Corne the
   corridor half-width is about 1.0 ≤ s = 1.2, so k = 1 is the last pass,
   at the edge.
3. **The slot is a known over-ceiling step.** It is a turn in the C14/C22
   sense: a step that the replay admits over the ceiling, with its feed
   scaled by `turn_feed_factor(c, w, area)` (`research_geodesic.rs:7316`)
   = `c / w` ≈ 0.219 for `w ≈ 1.0`. The chip load per tooth then holds
   (the C14 derivation). The 3D route already exists: the slice records
   its turns (`research_turns`) and `apply_slice_turn_feeds`
   (`adaptive3d/mod.rs:715`) writes the reduced feed on the moves that lie
   on each recorded segment. The slot is fed into that list as one turn
   per medial segment. Do not add a new feed path.
4. **The entry.** The level above is open over the strip: at Z 17 the
   strip is part of the big region, which C31 cut. So the first slot
   descends inside the strip from the cleared level above. That is a
   depth of `Depth/Pass` (6 mm), the same descent as every other entry of
   the slice. The entry style of the op applies; no new rule.
5. **Gate on geometry, not on a number.** The opening runs only when the
   seed does not fit (`ρ_max < R`). Both values come from the region. No
   new constant.

This adds a new over-ceiling exemption: a slot of length L at each level.
**It needs the lead's (and the operator's) ruling** before code, in the
same way as the C14 turns. The alternatives are the trochoid insert
(underived, excluded by ruling 2026-10-23) and leaving the band (the D20
state).

### 3.3 Pre-registration (Corne, "3D Rough 1", D20 + G-b + §3.2, the fix9 stock-map method)

Band areas over 0.8 mm, outside the outer wall, from `stockmap.py` /
`assert_fix9.py`, against D20 (R4 §1.5 table):

| Band | D20 | Predicted after the fix | Why |
|---|---|---|---|
| 3–4.5 mm | 620 | ≤ 62 (10 % of D20) | inside the corridor's reach: disc covers 0.51 to 8.49 mm from the wall |
| 4.5–6 mm | 511 | ≤ 51 | same |
| 6–9 mm | 820 | ≤ 208 | the outer 0.51 mm (8.49–9) stays, plus one stock-map cell (0.25 mm): (0.51 + 0.25) / 3 × 820 |
| **3–9 mm total** | **1951** | **≤ 321** | sum |
| 0–1.5 mm | 1185 | ≤ 1185 (not worse) | the corridor leaves 0.51 mm at the wall; the waterline traces the wall |
| 1.5–3 mm | 591 | ≤ 591 (not worse) | the disc reaches the wall + 0.51 |

The 10 % bound on 3–6 mm covers the stock-map cell (0.25 mm) at the
corridor ends and the taper risk of §2.1. If a taper splits the strip,
that piece falls to the shipped path (§2.1 risk), and the 3–6 mm figure
shows it.

Falsifiers:

- **F1 (gate only).** Run G-b without §3.2 first, as a separate column. P:
  the 3–9 mm band is ≥ 0.9 × 1951 = 1756 mm². If it falls well below that,
  §2.3 is wrong (the narrow loops do cut under the load) and §3.2 may not
  be needed.
- **F2 (time).** The slot is about L mm per level at 0.219 · F. With F
  4000 (R4 §2.1) that is about 876 mm/min, and the side passes run at F.
  P: the added time is `3 · L · (1/876 + 2/4000)` minutes. At L = 400 mm
  that is about 2.0 minutes on D20's 2760 s. The run reads L from the P1.2
  area.
- **F3 (load).** On the C18 oracle, every over-ceiling sample in the strip
  lies on a slot move that carries the reduced feed (the turn summary
  lists it). A side pass over the ceiling falsifies §3.2.2.
- **F4 (wanaka200).** No outer strip forms (R4 §1.5), so the output for
  wanaka200 is byte-identical to D20. A difference there means the
  corridor rule fires on something that is not a strip.

## 4. The shipped fix (G-OUTERBAND) — described only, not changed

Shipped output stays byte-identical until the lead and the operator rule.
A shipped fix would be:

1. **No second inset.** Give the 2D planner the region grown by `R − m`,
   as C31 does for the research arm, in place of the smoothed region that
   it insets by R again (`planner_outlines`). The guards of
   `research_grown_outlines` come with it.
2. **The gate reads the plan.** Replace the forecast
   (`clearing.rs:2844-2910`), which is a perimeter estimate of an inset by
   `R + 0.25` plus a dry run of the agent on the raw region, with G-b. The
   shipped arms plan under `RetractedByCaller` (the historical rule, no
   pass load), so the agent's narrow loops DO cut a slot in the strip at
   full feed, as contour_parallel does now. With G-b alone, AgentSearch and
   ContourSpiral cut the strip. No corridor opening is needed for the
   shipped arms; it is needed only where the pass load holds.
3. **A derived margin.** `PERIMETER_INSET_MARGIN_MM_FORECAST = 0.25`
   (`clearing.rs:2845`) and `PERIMETER_INSET_MARGIN_MM = 0.25`
   (`clearing.rs:2549`) are typed. The margin keeps a centre off the region
   boundary so that the boundary clip does not flip it (O5b/O5c). The
   region boundary is known only to the material grid. The marching
   squares put it within half a cell, and the lift's nearest read reaches
   half a cell. That is one planner cell (`max(R/6, tolerance)`, 0.5 mm on
   Corne). C31's measured-margin guard already uses this bound. The derived
   margin is therefore `cell`, not 0.25. Under G-b the forecast margin
   goes away with the forecast.
4. **The cost.** This changes shipped output wherever a region meets a
   wall (the double inset) and wherever the gate skipped a region that
   plans more than 15 mm. So `adaptive3d_emission_byte_parity` and
   `every_op_keeps_its_toolpath_g_opgolden` change and need a re-bless on
   a measured cause. Byte parity holds only while the fix stays behind
   `research`. Memory note: the operator roughs with contour_parallel,
   which has none of this, so the shipped fix matters for AgentSearch and
   ContourSpiral users only.

## 5. Item B: the C31 grow at edges that are not walls (Z 17)

### 5.1 Three kinds of edge, not two

R4 §7.2 calls the containment line a free edge ("no wall, no stock beyond
the clip"). The code and the stock map say the outer strip has three kinds
of edge:

| Edge | Planner stock beyond it | Real stock beyond it (within the disc's reach) | What the C31 grow by `R − m` puts in the 2D grid |
|---|---|---|---|
| The case wall | the wall | stock up to the wall | real stock (C31's intent) |
| The containment line (silhouette + 6 mm, tool-centre space) | **air**: `path.rs:835-868` clears every cell outside the boundary polygon in the planner stock (O5b) | **stock**: the 6–9 mm band from the wall stands 16–22 mm tall in every column but CP (R4 §1.5: 820 mm²), and D20's Z 17 discs cut it (18.2 mm left) | real stock that the planner's 3D stock calls air. This is NOT phantom in material terms |
| The stock-box edge (mesh bbox + 5 mm; `simulation.json` header) | none (the grid ends) | none: air | phantom: `R − m` of material beyond the real edge (C22 §11.3) |

So "grow only along walls" (R4 §7.2) is wrong at the containment line. The
grown band there is real stock within the disc's reach. If the 2D grid
reads it as air, a turn at the line meets stock that the plan does not see:
a load the planner hides, not a load it removes. Only the stock-box edge is
a true free edge.

### 5.2 Where the 124 descents are (measured, script with asserts)

`phase3/c38/classify_outer_descents.py` (`python3 -I`, asserts every
number here) reads `fix9/entries.json` (D20) and `fix9/corne_part.npz`:

| Class | Count | What stands beyond |
|---|---|---|
| Beyond the mesh bbox, in the 5 mm margin to the stock box: 0.55–4.6 mm from the stock-box edge | **45** | air (true free edge) |
| On the part grid, 1.5–3 mm from the silhouette | 6 | wall ends: the centre stands at the R limit, read on a 0.2 mm map |
| On the part grid, 3–4.5 mm from the silhouette | **58** | wall ends: the centre limit is wall + R = 3 mm |
| On the part grid, 4.5–6 mm from the silhouette | 15 | near the containment line (6 mm): real stock beyond |
| Total outside the silhouette | 124 | (R4's total; R4's 38 / 86 split used a mask resampled onto the stock grid, which also covers the margin beyond the mesh bbox) |

All 79 on-grid descents are at least 5.14 mm from the stock-box edge.

### 5.3 The design for item B (research arm only)

**Intersect the C31 grown outline with the stock XY box.** That is the
real stock footprint in the plane. `research_grown_outlines` keeps its grow
and its guards. The grown polygons are clipped to the stock rectangle
(`Adaptive3dGeometry::world_stock_xy_bbox`, or the planner stock bbox)
before `grown_planner_outlines`.

- At the stock-box edge the phantom band goes. The 2D tool-centre region
  then ends R inside the real edge, the disc reaches the edge exactly, and
  the arc ends where the material ends. A turn there reads air, as it is.
- At the containment line the grown band stays. It is real stock.
- At a wall nothing changes.
- No new constant: the stock box is an input.

Rejected alternatives:

- **"Grow only along walls"**, which needs a per-point wall test: it frees
  the containment line, where the stock is real (§5.1).
- **Seed the 2D grid from the planner stock** (`seed_from_material_mask`,
  the G-RESTLEVELS route): the planner stock reads air beyond the
  containment line (the O5b clear), so it frees the same edge wrongly.

**The planner-stock divergence at the containment band is a separate
finding, not C38 scope.** The planner's 3D stock reads the 6–9 mm band as
air while real stock stands there. The 3D entry and keep-down link
decisions read the planner stock, so they cannot see that band. The O5b
clear is shipped behaviour. I flag it for the lead and do not change it.

### 5.4 Pre-registration for item B (Corne, "3D Rough 1", Z 17, `entries.json` method, the same selection as the script)

- P-B1: of the 45 descents beyond the mesh bbox, at most 5 remain (≥ 90 %
  go). The turn at the stock-box edge now reads air.
- P-B2: the 79 on-grid outer descents change by at most 25 % (≥ 59 and
  ≤ 99). They are not at a stock-box edge: 64 are wall ends and 15 are at
  the containment line with real stock beyond. They belong to the open-arc
  join contract (§6), not to item B.
- P-B3: the 6–9 mm band at Z 17 does not get worse (D20 cut it at Z 17).
  The 3–9 mm figure of §3.3 is unchanged by item B.
- P-B4: wanaka200 has no vertical outer wall, so its stock-box edges meet
  the terrain. The descents within R of its stock-box edge, at every plane,
  fall. Count them with the script's rule before and after; there is no
  number to give before the run.
- Falsifier: if P-B1 fails (more than 5 of the 45 remain), the descents at
  the stock edge are not caused by the phantom band, and the cause is the
  arc order or the 3D stage.

## 6. Where C38 and C37 touch

C22's §13.6 assigns all 124 outer-strip descents to C38. By §5.2 that is
not right. Only the 45 at the stock-box edge are item B's. The 64 wall
ends at the outer wall (1.5–4.5 mm from the silhouette) are the same
mechanism as C37's inner-wall open-arc ends: an arc ends at the centre
limit R from a wall, and the next arc starts one stepover inward. The 15
at the containment line are open-arc ends at an edge with real stock
beyond, which is the same contract again. I propose:

- **C38 owns:**
  - the F-038 gate for the research arm (G-b);
  - the corridor opening (§3.2), behind a ruling;
  - the clip of the grown outline to the stock box (§5.3);
  - the 45 stock-edge descents at Z 17.
- **C37 owns** the open-arc join contract at every wall, outer and inner,
  and at the containment line: 74 + 65 + 123 inner, plus 64 + 15 = 79
  outer at Z 17. C37's instrument should classify the outer ones with the
  same silhouette rule as the script, so that the two counts add up.
- **Code places:**
  - C38 edits `clear_one_region` before the producer: the gate block
    (`clearing.rs:2844-2910`), and a stock-box clip on the output of
    `research_grown_outlines` at its call (`:2944`) or inside it after the
    grow. C22 owns C31, so C38 asks C22 before it touches
    `research_grown_outlines`. The least change is the clip at the call
    site.
  - The corridor opening goes where the field returns `None`, in the
    slice's producer switch (`research_slice_segments` / the fallback in
    `plan_one_outline`). It feeds its slot as turns into the existing C22
    turn list. That list is C22's structure, so the slot record must use
    C22's types unchanged.
  - C37 edits the field's arc joins and its record plumbing after the
    cleanup (C22 §13.6). The two sets of lines are disjoint.
- **Order effect:**
  - Item B changes the 2D region at the stock edge only, so C37's zones
    change shape there, not count (C22 §13.6 says the same).
  - The corridor opening adds new descents in the strip below Z 17: one per
    corridor piece per level for the slot entry, from the open level above.
    C37's instrument must read those as "corridor slot entries", not as
    open-arc ends. The slot record carries that tag.

## 7. What C38 could not do

- No run. §1.1 is read from an existing artefact. Every statement in §2
  and §3 is a prediction from the code at `b37cad1a4`.
- The strip length L: the files I read do not give it. P1.2 and F2 read it
  from the run.
- The 45 descents beyond the mesh bbox have no silhouette distance (the
  part map covers the mesh bbox only); the script gives their distance to
  the stock-box edge instead.
- I did not read C22's in-flight edits of `plan_piece` and
  `research_geodesic.rs`. If C22 adds a seed for a corridor, §2.2
  changes.
