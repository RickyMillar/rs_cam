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

**Result (D9, 2026-10-26, `phase3/c38/corne_d20_debug/`):** the CLI
writes the tracing lines to `stdout.txt`, not `stderr.txt`
(`grep_lines.txt` holds them). In op 1, region 1 is skipped at Z 11, 5 and
0.5 with `forecast_mm=0.0` and `area_mm2=1334.75`; 1334.75 / 3 ≈ 445 mm,
so it is the strip. Only region 2 opens. P1.1–P1.3 hold. The copy op is in
§7.

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

### 3.4 Verifier steps after the C38 code (read with the run log)

The tracing lines are in the CLI's `stdout.txt`. Run with the filter
`RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug`, as in §1.2.

1. **Op 1 strip at the C31 opening (G-RINGGROW, shortfall split).** Read
   the `C31 region opening` lines with `region=1` at `z=11`, `z=5` and
   `z=0.5`, and their `decision`. P: each says `decision="grown, no
   sweep"`.
   - "closing guard: shipped sweep" is the G-RINGGROW refusal on the ring.
   - "the shortfall inset split the outline: shipped sweep" is the taper
     split.
   - Either one means the strip falls to the shipped path, which cuts
     nothing in 3 mm, so §3.3 cannot hold for that level. Report it before
     any band figure.
2. **The gate is gone for the arm.** No `F-038: skipping micro-region`
   line remains in op 1 or the copy op. A region the plan judges short is
   counted by the new G-b judge (`skipped_f038` on the span, with the
   planned length as `forecast_cut_length_mm`).
3. **The copy op** (§7.3, P-C1): no copy region gives `decision="grown, no
   sweep"`, and the copy op plans no slot.
4. **The slot records:** every slot step carries the `Slot` kind and its
   reduced feed. The turn and slot counts are printed apart.
5. **The band and the descents:**
   - the 3–9 mm band, by §3.3 and `assert_fix9.py`'s method;
   - the Z 17 outer descents, by `phase3/c38/classify_outer_descents.py`
     on the new `entries.json` (§5.4).
6. **Shipped neutrality:** `adaptive3d_emission_byte_parity` without
   `research`, and the adaptive3d sentries (`adaptive3d/CLAUDE.md`).

### 3.5 As built (2026-10-26): the corridor opening is concentric loops, not a medial slot

The code (`adaptive3d/research_outer_band.rs`, `corridor_opening`; called
in `clearing.rs` `research_slice_segments` when the field returns `None`)
differs from §3.2 in one point.

- **The gate is as designed.** The 2D tool-centre region (the piece inset
  by R) is not empty, and its inset by R more is empty: no centre
  clearance of R, so the seed does not fit.
- **The plan is the loops of the tool-centre region at `k · s`, outside
  in, not a medial slot plus side passes.**
  - A medial line by offset finds only the widest part of a corridor whose
    width varies. The Corne strip is wider at the concave corners, where
    the containment line bulges.
  - The loops are the shipped narrow-gate geometry. Every edge is split to
    steps of at most `s`, under `turn_bound`.
  - **Every step is a slot** (`SlicePlan::slots`, `TurnKind::Slot`). The
    replay admits it as a turn. The 3D stage feeds it at
    `turn_feed_factor(c, w, area)` of its MEASURED engagement: `c / w`
    over the ceiling, and the full feed where the step holds the ceiling.
    A slot record with `steps_over = 0` held the load.
- **In the Corne strip** (centre width 2ρ ≈ 2.0 mm < 2s = 2.4) the plan is
  the two edges. The first edge is the slot (w ≈ 1, feed ≈ 0.219 F). The
  second bites about 2ρ / D ≈ 0.33, so its feed is about 0.66 F.
  - Feed-time: `L/F · (1/0.219 + 1/0.66)` ≈ 6.1 L/F, against 6.6 L/F for
    the medial slot plus two sides.
- **Changed falsifiers:**
  - F3 becomes: every over-ceiling sample in the strip lies on a slot step
    that carries the reduced feed. A slot step with no reduced feed and
    w > c falsifies it. The second edge is expected over the ceiling.
  - F4 changes. The corridor rule is geometric, so it runs on every piece
    with no centre clearance of R. wanaka200 and the inner necks of Corne
    have such pieces, where the agent fallback ran before. wanaka200 is
    therefore not byte-identical to D21. Count the `C38 corridor opening`
    info lines per run (loops, slot steps, cut mm, centre area). Report
    the area they cut outside the outer strip separately.

### 3.6 Pre-registration of the corridor openings (lead, 2026-10-26: the rule is accepted for every piece with no centre clearance of R)

Each opening writes one info line, `C38 corridor opening`, with `loops`,
`slot_steps`, `cut_mm` and `centre_area_mm2`. The line carries no Z or
region, so read each one against the `C31 region opening` / `C38 G-b`
lines just before it. A loop set at `d = k·s` exists only while
`k·s < R`, so a piece has at most `⌈R/s⌉` loop sets: 2 on Corne op 1
(s 1.2, R 3), 3 on the Corne copy op (s 0.6, R 1.5), 2 on wanaka200
(s 2, R 3). So `cut_mm ≤ ⌈R/s⌉ ×` the perimeter of the piece's
tool-centre region.

**Corne "3D Rough 1":**
- P-O1: the outer strip opens at Z 11, 5 and 0.5. That is one opening
  per piece of region 1; with C31 accepting the ring, one per level.
- P-O2: the strip's centre width is about 2.0 mm < 2s, so only `k = 0`
  exists. That is 2 loops per ring (the exterior and the hole) and
  `cut_mm` ≈ 2 × the strip length ≈ 2 × 445 ≈ 890 mm per level (± 15 %
  for the corners and the margin).
- P-O3: openings elsewhere in op 1 (inner necks) are reported apart from
  the strip, as the lead asks: count, `cut_mm`, `centre_area_mm2`, with
  the Z and the region from the lines around them. I give no number for
  them before the run.

**Corne "3D Rough 1 (copy)":** P-C1 holds as written (§7.3). No copy
region opens, so it has no corridor opening.

**wanaka200 "2 Back Rough"** (the D20 log, `fix7/wanaka200`: four
levels):
- P-W1: no opening at Z 19.54, 14.08 or 8.62. Each level is one region of
  40100 mm², "grown, no sweep", with a centre clearance far over R.
- P-W2: openings only at Z 4.0, and only in region 2, the one F-038
  skipped in D20 (forecast 0). Its inset by `R + 0.25` was empty, so its
  centre width is under `2R + 0.5`, and under the C31 margin no centre
  clearance of R remains. The count is the number of its pieces: 1 to 3.
  It is 0 if C31 refuses region 2 (closing guard or split: then the
  shipped outlines are double-inset and the piece has no centre region).
  - Region 1 (the frame ring, closing guard) and region 3 (split) keep
    the shipped outlines and get no opening, as in D20.
- P-W3: the total `cut_mm` of the wanaka200 openings is ≤ 2 × the
  perimeter of their centre regions (two loop sets at most). It is also
  under 3 % of the op's D20 walk (62466.8 mm, the sum of
  `agent_walk_cut_length_mm` over the four levels), so ≤ 1874 mm. The 3 %
  is a stated bound, not a derived number: if the run exceeds it, the
  openings are wider than F-038's 0 forecast implies, and that is
  reported.

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

### 6.1 Agreed with C22 (2026-10-26, before the code)

1. **The gate:** `min_region_cut_length_mm > 0.0 &&
   !is_research_arm(..)`. Accepted.
2. **The G-b judge:** one cfg block after the outline loop in
   `plan_and_emit_region`. There is no judge in `pick_region_plan`: both
   scratch runs judge alike. **Addition from C22:** take
   `(turns.planned, turns.records.len(), core_zones.len())` of
   `ctx.research_turns` / `ctx.research_core_zones` before the outline
   loop. On a skip, truncate the records back and restore `planned`, as
   `pick_region_plan` does for the loser. Without this, the skipped
   region's turns read as unfed, and the core sentry reads a zone that was
   never cut.
3. **The stock-box clip** at the `research_grown_outlines` call site.
   Accepted. C22 does not touch the call site or the function.
4. **The slot types are C22's,** and they land after C35 and before my
   slot code:
   - `SlicePlan { .., slots: Vec<(P2, P2)> }`;
   - `plan_one_outline` passes `turns ++ slots` to `slice_cleanup` and
     `slice_turn_records`;
   - `SliceTurns.kinds: Vec<TurnKind { Turn, Slot }>`;
   - `apply_slice_turn_feeds` splits the summary by kind, and `Research3d`
     gains `slots: TurnSummary`.

   The 2D C14 types do not change. The corridor fn returns
   `SlicePlan { segments, turns: vec![], slots, core: None }`.
- **C37's stage-1 lines** in `clearing.rs`, none in the four places above:
  - the record block in `plan_one_outline` after the C35 core-zone record;
  - the record ranges in `pick_region_plan`;
  - the cfg fields of `ClearZLevelContext`.
- **A region that falls back** to the shipped path (G-RINGGROW) gets the
  shipped smoothing and sweep and cuts nothing in a 3 mm ring. It counts
  in C38's outer-strip figures.

## 7. The copy op, "3D Rough 1 (copy)" (D9 debug run, 2026-10-26)

### 7.1 What the copy op is

From `corne_job_corrected_c12_agent_search.toml`:
- Tool 1 is a **3 mm flat end mill** (R 1.5). Stepover 0.6, Depth/Pass 3,
  axial leave 0.5, F 2429, plunge 375. `min_region_cut_length_mm` is 15.
- `stock_source = "from_remaining_stock"`: it is the rest rough after op 1.
- The boundary is the silhouette with `containment = "center"`,
  `offset = 3.0`. Op 1 has 6.0.
- Its strategy is `agent_search`, so the env variable puts the research arm
  on it too.
- The pass ceiling is `c = 0.2189`, the same as op 1, because `s/R` = 0.4
  for both.

### 7.2 Where its three regions are, and where its cut comes from

- **The log** (`phase3/c38/corne_d20_debug/grep_lines.txt`): F-038 skips
  every region at every level. That is 3 regions × Z 14, 11, 8, 5, 2 and
  0.5 = 18 skips. The areas are 258–281, 72–81 and 258–281 mm²
  (611–621 mm² per level), and the forecast is 0 in every case. There is
  no C31 line, because the gate skips each region before C31 runs.
- **The trace** (`tp_3_3D_Rough_1__copy_.json`) shows the following at all
  six levels:
  - `planner_cut_mm` 0;
  - `agent_walk_cut_length_mm` 0;
  - `perimeter_sweep_length_mm` 0;
  - `dropped_short_regions_f038` 3.

  The only producers are the six `Waterline cleanup` spans (2 contours at
  Z 14–5, 84 contours at Z 2 and 0.5) and their 176 plunge entries. So
  **all the copy op's cut comes from the waterline cleanup**: your 7060 mm
  of cut, and the 16869 mm cutting distance of the trace with plunges and
  links. No region body runs.
- **The place** (`phase3/c38/copy_op_regions.py`, `python3 -I`, asserts):
  - The script rebuilds the copy op's level grid from the D20 stock map
    after op 1. It uses the 3 mm flat's tool-centre floor, the
    `cell_has_material` test and the O5b boundary clear at silhouette +
    3 mm.
  - At Z 14–5, at least 95 % of the material is outside the silhouette. At
    Z 2 and 0.5 it is at least 85 %, because small floors inside the case
    join there. The median wall distance is 2.2 mm.
  - So the regions are the **outer strip again**: the tool-centre band
    from wall + 1.5 to the containment line at wall + 3, 1.5 mm wide.
  - The rebuilt area (316–364 mm²) is a lower bound. The part map does not
    cover the margin beyond the mesh bbox. At 611–621 mm² / 1.5 mm, the
    logged strip is about 410 mm long. The split into 3 pieces fits the
    necks, where the band pinches at the corners and tapers. The rebuild
    finds several fragments too.
- **Why the forecast is 0:** the perimeter inset is `R + 0.25` = 1.75 mm,
  and the agent dry run insets by R = 1.5 mm. Each of those empties a band
  1.5 mm wide.

### 7.3 What G-b plus the corridor opening does to the copy op, and what it should cut

The copy op's input is op 1's remaining stock. With C38, op 1 changes
first. So predict the copy op after op 1 is fixed. That is the run D9
makes: one env variable puts the arm on both ops, so the copy op cannot be
changed alone.

- **After op 1 with C38** (§3.3): op 1 cuts the outer band from 0.51 to
  8.49 mm from the wall at every level. It leaves a skin about 0.51 mm
  thick at the wall, plus the axial leave.
- **The copy op's level grid then** holds only the cells whose 3 mm disc
  meets that skin. Its centres stand from wall + 1.5 to about wall + 2.0,
  a band about 0.5 mm wide.
  - C31's smoothing (inset 0.3, RDP 0.3) and its margin (≥ one copy cell,
    0.25 mm; the nominal 0.55) empty a band that narrow. The decision is
    "no smoothed or grown outline: shipped sweep".
  - The shipped sweep insets by 1.75 mm, which leaves nothing.
  - Under G-b each region plans 0 mm, so it is skipped and counted, as now.
- **So the copy op should cut what it cuts now:** the waterline at the
  wall. That takes the 0.51 mm skin at a radial load of 0.51 / 3 = 0.17 of
  D, under `c` = 0.219, which is within the pass load. That is the right
  job for a rest rough that has no room in the strip.

**Pre-registration for the copy op (Corne, the full job, both ops on the arm with C38):**

- P-C1: 18 region skips again (3 × 6 levels), each with a planned length
  of 0, or C31 lines with "no smoothed or grown outline" / "shipped
  sweep". No copy region gives "grown, no sweep".
- P-C2: the copy op's cut length and its plunge entries stay within 10 %
  of today (the waterline is the same contour; the stock it meets is
  thinner).
- P-C3: the band outside the wall from 0 to 1.5 mm after the full job is
  no worse than the D20 full job. The skin is cut by the copy's waterline.
- **Falsifier F-C:** a copy region opens ("grown, no sweep"), or the copy
  op plans a corridor slot. Either one means op 1 left more than the
  0.51 mm skin, and §3.3's band figures are then wrong as well.

If op 1 were NOT fixed (today's D20 op 1), the copy op's corridor would be
the 1.5 mm strip inset by about 0.55 on each side: about 0.4 mm wide. The
seed needs a centre clearance of 1.5, so it gets the corridor slot. The
slot runs at 2429 × 0.219 ≈ 532 mm/min. It is about 410 mm × 6 levels =
2460 mm of slot (4.6 min), with 18 entries, and it clears 0.55–3.95 mm
from the wall. That case cannot be run alone with one env variable
(above), so it is not pre-registered as a run.

### 7.4 Risks shared with op 1

- **G-RINGGROW:** below Z 17 the outer strip is a ring round the case.
  The C31 closing guard can refuse it, and then the region gets the
  shipped sweep, which cuts nothing in the strip. Read it in the first run
  after the gate change: the `C31 region opening` lines for op 1 region 1
  at Z 11/5/0.5 and their decision. This D9 run at b37cad1a4 cannot show
  it, because the gate skips the strip before C31.
- **The shortfall split:** a strip that tapers can split under the
  shortfall inset (§2.1). Both ops have tapers at the stock-box corners.

## 8. What C38 could not do

- No run of the C38 code. §1.1 is read from an existing artefact, and
  §7 from the D9 debug run at b37cad1a4. Every statement in §2
  and §3 is a prediction from the code at `b37cad1a4`.
- The strip length L: the files I read do not give it. P1.2 and F2 read it
  from the run.
- The 45 descents beyond the mesh bbox have no silhouette distance (the
  part map covers the mesh bbox only); the script gives their distance to
  the stock-box edge instead.
- I did not read C22's in-flight edits of `plan_piece` and
  `research_geodesic.rs`. If C22 adds a seed for a corridor, §2.2
  changes.

## 9. D22 (c38a alone): why F1, P-C1 and P-B1 failed, and the new hard bar (2026-10-26)

Read only. The scripts are in `phase3/c38/d22_why/`: `extract.py`,
`why.py`, `why2.py`, `stockedge.py`, `drops.py` and `assert_why.py`. Each
asserting script prints `ASSERTS OK`. They read the D20 run (fix7) and the
C38a run (`corne_c38a`) and use the C35 run (fix12) for runtimes. "The
strip" here means op 1 cutter centres 3–6.2 mm outside the silhouette at
the strip levels Z 11, 5 and 0.5.

### 9.1 F1 is falsified: the strip opens under the pass load, by passes ACROSS it

| Op 1 in the strip, levels Z 11/5/0.5 | D20 | C38a |
|---|---|---|
| clearing-cut length | 23.1 mm | 4547.7 mm (15159 samples) |
| radial engagement, median | 0.921 | **0.0** |
| samples over c = 0.219 | 56 | 1176 (7.8 %) |
| plunge-entry samples | 0 | 0 |
| moves (from the sample runs) | — | 3049; length median **1.2 mm = s**; 1908 (63 %) run mostly ACROSS the strip (\|Δ wall distance\| / length > 0.7; median 0.8) |

§2.3 said the first pass through a closed strip is a slot "whatever the
producer". That is true only for a pass ALONG the strip. A pass ACROSS
the strip meets new stock only over its advance along the strip, so its
swept width is that advance, not D. The 2D planner, under the pass load,
opened the strip that way. Short steps run from one side of the corridor
to the other. Each is about one stepover long, and the next is a little
further along. The median radial is 0, and 92 % of the samples hold the
ceiling.

The 3D trace does not tag the 2D producer (the markers are swallowed in
3D), so the script cannot name it. By the code, it is the agent fallback:
the narrow loops refuse the along-strip spans, and the mop and the residue
walk pick up the rest in any direction.

- **The strip was 3 pieces per level** (op 1 region 1 at Z 11 has three
  `F-038: dropped short cut groups` lines: 19/39, 10/23, 8/18 groups).
- **The corridor is 1.39 mm wide, not 2.0.** The C31 line gives
  `centre_area_mm2 = 617.9` against a strip length of about 445 mm.
  Smoothing plus margin take about 0.8 mm off each side, not 0.51.
- **What stands after it:** 3–9 mm 794 mm² (3–4.5 313, 4.5–6 180, 6–9
  301). The disc reaches 0.8 to 8.2 mm from the wall. 8.2–9 is left
  (0.8 / 3 × 820 ≈ 219, plus the stock-map cell), and the nibble does not
  reach every corridor edge.
- **The cost:** op 1 runtime C35 1451.7 s → C38a 1750.7 s, **+299.0 s**,
  from 3049 short reversing moves.

### 9.2 P-C1 fails because its premise failed; the copy op is RIGHT to open

P-C1 assumed op 1 leaves only a 0.51 mm skin (the slot design, §3.3). In
c38a op 1 leaves 304 mm² at 1.5–3 mm and 313 mm² at 3–4.5 mm. Both bands
are within the copy op's reach (3 mm tool, centres to 3 mm, disc to
4.5 mm). So the copy op's level grid holds material, and its region 1
(about 206 mm², centre area 75–87 mm²) opens with "grown, no sweep" at
each level. That is correct: the stock is there.

| Copy op, outside the silhouette | D20 | C38a |
|---|---|---|
| clearing cut: length / removed | 4094.7 mm / 181.9 mm³ | 7044.3 mm / **3238.7 mm³** |
| keep-down links: length / removed | 0 / 0 | 1378.8 mm / **2475.9 mm³** |
| plunge entries: length / removed | 7.2 mm / 0 | 1163.7 mm / 1239.8 mm³ |
| runtime (whole op) | C35 1305.2 s | 1694.3 s (+389.1 s) |

The copy op now does real work: 3239 mm³ cut, against 182 before. But
its 2D plan in a corridor under 1 mm wide is hundreds of tiny groups.
Region 1 has 610 groups over six levels; the F-038 group filter drops 509
of them (`drops.py`: 287 of 310 at Z 5). And its links remove almost as
much as its cuts (§9.4).

### 9.3 P-B1: the clip did its job; the class I counted was too wide

`stockedge.py` splits the Z 17 descents beyond the mesh bbox at R (3 mm)
from the stock-box edge. Within R, the C31 grow put phantom material past
the edge (§5.1). At R or more, the disc stays in the box, and the clip
cannot touch the descent.

| Beyond the mesh bbox | D20 | C35 | C38a |
|---|---|---|---|
| within R of the stock edge | 32 | 32 | **1** |
| at R or more | 13 | 13 | 13 |

The clip removed 31 of the 32 descents it could act on. P-B1 (≤ 5 of
45) counted the 13 at R or more, which no stock-edge rule can move. They
are wall ends and containment ends in the 5 mm margin beyond the mesh
bbox, the C37 class. Restated: **P-B1′: at most 2 within R remain. It
holds (1).**

### 9.4 The new hard bar: the copy op's held links cross ground that 3D never cut

The bar: "a fed link cuts stock it was planned to cross in air or on cut
ground: link move 8460 removes 15.43 mm at (3.0, −71.6, 0.50). 301 of 4596
link samples on 193 link moves remove more than 3.30 mm".

`why.py` and `why2.py` reproduce the bar with the same selection (copy op
link samples with axial engagement over 3.30 mm): 301 / 4596 samples on
193 moves.
- **Where:** all 301 lie at a centre wall distance of 1.5–3 mm, the copy
  op's own strip, inside its containment (3 mm). The planner stock at the
  link's centre is not O5b-cleared. So this is **not** G-CLIPAIR.
- **When:** they are at Z 0.5, 2.01 and 8.01.
- **What came before:** only 14 of the 301 have a copy-op clearing sample
  of the same level within R of them before the link. For 287, no cut of
  that level ever passed there. The link removes the full column that op
  1 left (15.4 mm at Z 0.5).
- **D20:** 0 such samples. The copy op had no links outside the
  silhouette.

**The mechanism, by the code order** (`plan_one_outline` →
`plan_and_emit_region`):
1. The 2D replay (the research cleanup) stamps every group of the plan
   on its grid and admits each held link over the ground those stamps
   cleared.
2. AFTER the replay, the F-038 group filter drops each group that cuts
   under 15 mm. In the copy op that is 509 of 610 groups. The 2D grid
   still holds them as cut.
3. `HeldLinks` (C22/C26) emits each admitted 2D link as a 3D keep-down
   link at the level. It checks that the link stays at the level, but it
   does not read the planner stock. So a link planned over a dropped
   group's ground ploughs the stock that group would have cut.

This is the "planner stock is the emitted path" rule
(`adaptive/CLAUDE.md`: "Do not stamp stock no emitted move cuts"),
broken between the replay and the filter. The rule has been in the arm
since C22. It shows now because the copy op's corridor plan is mostly
tiny groups.

The falsifier, a run and no code: the same project with the copy op's
`min_region_cut_length_mm = 0`, saved as
`phase3/c38/corne_c38_mrcl0.toml`. It is the Corne job
(`corne_job_corrected_c12_agent_search.toml`) with only line 248, the copy
op, changed from 15.0 to 0.0. The model path is absolute. With 0, both the
G-b judge and the F-038 group filter are off for the copy op. P: the bar goes
(0 samples over 3.30 mm), and the copy op's entries rise by up to 509.

### 9.5 Proposal

1. **Fix the hard bar where the link leaves.** This is C22's code
   (`HeldLinks` in `plan_and_emit_region`). A held link goes out
   keep-down only when the planner stock along its swath has no
   material above the level plus the stay-down clearance. That is the
   test `plan_entry` already makes for every 3D entry ("no link feeds
   down into stock"). Otherwise it goes out as an entry, and
   `plan_entry` decides. This closes the bar for any cause: dropped
   groups, or any later 2D/3D divergence. No new constant.
   - Measured cost bound: the 193 link moves become entries at most.
2. **Stop the cause in C38's arm (my code, after 1).**
   - In a research region, the F-038 group filter is redundant with the
     G-b judge, which already removes a region whose whole plan is short.
   - Inside a planned region, a group can drop only if no later step of
     the plan reads its stamps. That is not known after the replay.
   - **Proposal:** the research arm does not run the F-038 group filter
     inside a planned region. The short groups stay, each with its entry.
   - Cost bound: up to 509 entries in the copy op, plus 37 per level in
     op 1's strip.
   - If (1) alone holds the bar, I prefer to keep the filter and let (1)
     lift the affected links. That costs at most 193 entries, against up
     to 546 kept groups. D23 shows which is smaller.
3. **The copy op opening stays** (§9.2: real stock, 3239 mm³ cut). With
   part 2 (D23), the copy op's sub-tool corridor gets the corridor
   opening (1–3 loops per piece) in place of hundreds of tiny agent
   groups. So its group count, its dropped groups and its links fall
   together.
4. **The slot against the nibble.** §9.1 shows that a load-holding plan
   exists without the slot exemption. The slot earns its exemption only
   if it is better. D23 (with the slot) against D22 (the nibble) decides.
   Pre-registered for op 1 in D23:
   - **The band:** 3–9 mm ≤ 400 mm². With the 1.39 mm corridor the disc
     reaches 0.8–8.2 mm: 6–9 ≤ (0.8 + 0.25) / 3 × 820 = 287, plus
     ≤ 10 % of the 3–6 mm bands (113).
   - **The runtime:** op 1 ≤ 1651 s (C38a's 1750.7 − 100). The two edge
     loops feed at c/w: edge 1 w ≈ 1 → 0.22 F; edge 2 bites 1.39 / 6 =
     0.23 ≈ c → about 0.95 F. Feed time about 120 s for 3 × 445 mm.
   - **The rule:** if D23's op 1 misses either the band or the runtime
     bound, drop part 2 and keep the nibble (no exemption).
5. **Restated predictions for D23:**
   - P-B1′ (≤ 2 within R of the stock edge) holds.
   - The copy-op hard bar holds only if (1) or (2) is in. D23 has
     neither, so **D23 will show the bar again** unless the corridor
     opening removes the copy op's tiny groups.
