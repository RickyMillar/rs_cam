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

## 10. D23 (the full stack): the failed C38 predictions, and the slot against the nibble (2026-10-26)

Read only. The scripts are in `phase3/c38/d23_why/`: `extract.py`,
`why.py`, `slotfeed.py`, `band_any.py`, `stockedge.py`, `overbound.py`
and `assert_d23.py`. Every asserting script prints `ASSERTS OK`. The
inputs are the fix13 runs: `corne` = spiral_headon, `corne_pick` =
spiral_pick, the like-for-like column of C38a. "The strip" means op 1
cutter centres 3–6.2 mm outside the silhouette at Z 11, 5 and 0.5.

### 10.1 The slot steps ran at the FULL feed: the slot as built breaks the operator's ruling

`slotfeed.py`, D23 spiral_pick, op 1 in the strip:

- **Over the ceiling, at full feed:** 6049 clearing samples, and 2940 of
  them read over c = 0.219. **All 2940 run at F 4000.** Only 6 strip
  samples carry a reduced feed (750, the plunge rate).
  - Over the oracle bound c + 0.15 = 0.369 (`overbound.py`), the slot
    runs have 2468 samples (40.8 %). The nibble (C38a) has 524 (3.5 %).
- **Why the slots are unfed:** the emitted moves in the strip are 360,
  median 2.26 mm long, and 267 are longer than one slot step
  (s = 1.2 mm).
  - `corridor_opening` splits each ring edge into collinear steps of at
    most `s`. The 3D segment merge (RDP) joins collinear steps back into
    one move.
  - `apply_slice_turn_feeds` lowers the feed of a `Linear` move only when
    its start and its target lie on ONE recorded segment. A merged move
    spans several slot steps, so it matches none.
  - So the slot is "admitted over the ceiling" by the 2D replay but cut
    at the full feed. That is my defect: a C14 turn is one straight
    segment, but my slot is a chain of collinear steps.
- **At the ruled feed** (F × c/w on every over-c strip sample, swept-width
  form): the strip takes **+36.0 s** more.

### 10.2 The slot against the nibble (the D23 rule)

| Op 1, Corne | nibble (C38a) | slot (D23 spiral_pick) | bound |
|---|---|---|---|
| 3–9 mm band | 794 mm² | **455 mm²** | ≤ 400: **missed** |
| op 1 runtime (fix13 summaries) | 1750.7 s | **1468.4 s** (≈ 1504 s at the ruled feed) | ≤ 1651 s: holds |
| strip samples over the oracle bound | 524 (3.5 %) at full feed | 2468 (40.8 %), unfed | — |

The pre-registered rule says that if D23 misses either bound, the slot is
dropped and the nibble kept. **It misses the band bound, so by the rule
the slot is dropped.**

Head to head, the slot beats the nibble on the band and on the runtime.
But the band miss is not the slot's doing (§10.3), and as built the slot
runs over-ceiling steps at the full feed (§10.1). That breaks the
operator's ruling ("a counted over-ceiling step at feed × c/w").

**Proposal: remove the corridor opening** (`corridor_opening`, its
`closed_steps` helper, and the `or_else` in `research_slice_segments`).
- The nibble is the load-holding default. It needs no exemption and
  keeps 96.5 % of its strip samples under the oracle bound.
- If the lead wants the slot's gains, it needs a new pre-registration
  after two changes:
  - fed slots (one recorded segment per emitted move, or a match by
    distance to the slot chain);
  - a printed slot summary (§10.6).
- The G-b gate, the region judge and the stock-box clip stay. They do not
  depend on the slot.

### 10.3 Why the band is 455, not ≤ 321 (P) or ≤ 400 (§9.5): the strip at the stock-box edge is never planned

`band_any.py` splits the standing 3–9 mm cells by whether an op-1 cutter
centre below Z 17 came within R of them, and by their distance to the
stock-box edge:

| | slot D23 | nibble C38a |
|---|---|---|
| 3–9 mm band | 455 | 794 |
| no op-1 centre below Z 17 within R | **440** | 739 |
| ... of which within 6 mm of the stock-box edge | **403** | 590 |
| reached by a centre, still standing | 15 | 56 |

The band that stands is where nobody plans. It is mostly the strip along
the sides where the case wall is close to the mesh bbox.
- There the stock-box edge is 5 mm from the wall, so the strip is only
  about 2 mm wide in tool-centre space (wall + 3 to the edge).
- The C31 smoothing (0.3) and margin (≥ one cell, about 0.51 on Corne)
  take about 0.8 mm off each side, so no 2D tool-centre region is left.
- So neither the field, the slot nor the nibble plans those cells.

The pre-registration assumed a 3 mm strip everywhere. The cause is C31's
margin in a strip narrower than about `2R + 2 × 0.8` in centre space at
a free edge. That is C22's C31 code, and a separate item (§10.7). Both
the slot and the nibble reach the rest.

### 10.4 P-O1 and P-O2: the strip is four pieces, and the corridors are open strips, not a ring

The fix13 log has four `C38 corridor opening` lines per op-1 level. Each
has `loops=1`. Their cuts are 63.0 + 262.4 + 125.0 + 128.3 = 578.7 mm,
and their centre areas 55.7 + 217.0 + 58.3 + 67.7 = 398.7 mm².
- The strip is not one ring. The stock-box sides (§10.3) cut it, so the
  grown outline splits into pieces (`inset_piece_outlines` splits at the
  necks).
- A piece's centre region is an open sliver. Its one exterior loop runs
  along both edges, which gives `loops=1`.
- The corridor pieces hold 398.7 mm² of C31's 617.9 mm² centre region
  (C38a log). The rest is in pieces where the seed fits (the concave
  corners), which the field plans, and in the parts that no centre region
  holds (§10.3).
- So the corridor loops cover about 578.7 / 2 ≈ 289 mm of strip, not
  445. P-O2 (890 mm per level, one ring) was wrong because it assumed a
  ring.

### 10.5 P-B1 (18 for headon) and P-B2 (101 for headon)

`stockedge.py` on `entries_d23.json`:

| Beyond the mesh bbox | D20 | C35 | D23 headon | D23 pick |
|---|---|---|---|---|
| within R of the stock edge | 32 | 32 | **1** | **1** |
| at R or more | 13 | 13 | 17 | 13 |

P-B1′ (≤ 2 within R, §9.3) holds in both D23 columns. The 18 of headon
are 1 + 17. The 17 at R or more are the C37 class (wall and containment
ends in the bbox margin). The headon column has 4 more there than pick
(17 against 13), and P-B2's 101 against 87 is the same difference between
the columns, on the grid. I did not trace which headon rule adds them. Both are
C37's (§6), not the stock-edge clip's. The old P-B1 / P-B2 numbers
counted classes C38 does not own.

### 10.6 P-W2: the wanaka200 opening is on a region with the shipped outlines

The one wanaka200 opening (38 slot steps, 27.0 mm, centre 17.0 mm²)
follows the Z 4.0 line "the shortfall inset split the outline: shipped
sweep".
- §3.6 assumed that a region on the shipped path has no 2D centre region
  ("double inset"). That is wrong for a region wider than `2R + 0.6` in
  places. `planner_outlines` gives the smoothed region, and the 2D
  planner insets it by R once, so a piece can keep a small centre region
  with no clearance of R. The corridor rule then fires on it.
- Region 2 (F-038's forecast 0 in D20) is now G-b-skipped (planned 0):
  its grown piece has no centre region.
- P-W1 and P-W3 hold.

**Slot records print nowhere:** `Research3d.slots` exists, but no CLI or
test line prints it. If the slot is kept, my next change adds one info
line `C38 slot summary` (planned, records, fed, unfed, steps over) where
`apply_slice_turn_feeds` returns its `(turns, slots)` pair in
`adaptive3d/mod.rs`. That is C22's line, so I ask C22 first. If the slot
is removed, nothing is needed.

### 10.7 Two more findings from the D23 samples

- **G-CLIPAIR, measured. It caused the spiral_pick export refusal.**
  - What: 130 copy-op clearing samples cut deeper than 1.5 × its
    Depth/Pass (4.5 mm), up to 12.0 mm. 127 of them have their centre at
    3.2 mm outside the silhouette, which is the copy op's containment
    line (`why.py`). They are at Z 11.01, 5.01 and 2.01.
  - Mechanism: the disc reaches 4.7 mm from the wall, into the 3–4.5 mm
    band that op 1 left standing (§10.3). The copy op's planner stock had
    those cells as air: the O5b clear empties every cell beyond its 3 mm
    boundary. So the planner cut full depth along its containment line
    beside a 9–12 mm wall of stock it did not see.
  - Consequence: the copy op's 200 µm deflection and the D23 pick export
    refusal, and the "crosses standing material deeper than planned"
    triage line.
  - A fix is G-CLIPAIR's (shipped behaviour). The research arm could
    carry it first.
- **The free-edge strip (§10.3) is the larger part of the band in both
  runs.** It needs C31 (C22's code) to plan a strip of about 2 mm centre
  width at the stock edge. Example: no smoothing inset where the edge is
  the stock box, since there is no staircase from a wall there. That is
  a C22/C31 item. I do not propose a number.

### 10.8 For C22 (a C31 item after C22a): the strip at a free stock edge gets no 2D centre region

On Corne, op 1 leaves 440 of the 455 mm² of the 3–9 mm band where no
op-1 cutter centre came within R below Z 17. 403 of those 440 mm² lie
within 6 mm of the stock-box edge (`d23_why/band_any.py`; the nibble run
C38a has the same pattern, 590 of 739). Along the sides where the case
wall is near the mesh bbox, the stock-box edge is 5 mm from the wall. So
the outer strip there is only about 2 mm wide in tool-centre space (wall
+ R to the box edge).

`research_grown_outlines` first smooths the region (inset
`SIMPLIFY_INSET` 0.3, then RDP 0.3), then insets it by the shortfall to a
measured margin of one planner cell (about 0.51). In a 2 mm strip that
leaves no 2D tool-centre region, or one too thin to keep after the piece
split. So no producer (field, nibble or slot) plans those cells, and they
stand at full height below Z 17.

The edge there is the stock-box edge, a free edge with air beyond. It is
not a wall or a staircase, so the reasons for the smoothing inset and the
margin (O5b at a wall, the marching-squares staircase) may not hold
there. The item: let C31 give a 2D centre region to a strip that is about
2 mm wide in centre space at a free edge, with no new constant. Then
measure the 440 mm² again.

### 10.9 For the lead: does the merge defect hit the C22 turns? (read only)

- **On the C12 fixture: no.** `phase3/d23/10_c12_heavy.txt` prints the
  turn summary per arm. For `geodesic_local_spiral_headon` it reads
  "planned 84, cut 83, … fed 83, unfed 0, full feed with no move 0". Every
  arm with turns reads `unfed 0` and `fed = cut`.
  - Mechanism: a turn is one straight step between two arc ends at an
    angle. The RDP keeps a corner, so the emitted move is the turn
    segment, and `apply_slice_turn_feeds` matches it.
  - My slots were chains of COLLINEAR steps, which the RDP merges. That is
    the difference.
- **On Corne: not provable from the artefacts.** The CLI does not print
  `Research3d.turns`, the same gap as for the slots. `d23_why/turnfeed.py`
  reads D23 headon op 1 off the strip:
  - 6687 clearing samples carry a reduced feed (p50 1519 mm/min, on 387
    moves of median 0.86 mm), and only 95 of them read over c.
  - 2014 samples read over the oracle bound (0.369), and 1943 of those
    run at the full feed, on 568 moves (519 of them no longer than the
    turn bound).
  - Whether those 519 short moves are turns, or mop and wall steps, the
    samples cannot say (no producer tag).
- **Proposal:** the log line of §10.6 prints both summaries (turns and
  slots: planned, cut, fed, unfed, unmatched) per op in a CLI run. Then
  this check is one grep. The line goes in C22's `mod.rs`; C22 or I can
  add it.

## 11. G-CLIPAIR in the research arm: design (2026-10-26; for approval, no code)

### 11.1 The defect, measured

`adaptive3d/path.rs` (the boundary clear, O5b; 835-868 at b37cad1a4)
empties every planner-stock cell whose centre lies outside
`params.geometry.boundary`, the tool-CENTRE containment polygon. But a
disc whose centre is inside the boundary reaches R beyond it, and the
real stock there stands. So the planner reads air where the cutter cuts
stock. On D23 (`d23_why/clipair_base.py`), counting cuts deeper than 1.5
× Depth/Pass:

| | op 1 (limit 9.0 mm) | copy op (limit 4.5 mm) |
|---|---|---|
| deep samples, headon / pick | 654 / 627 | 129 / 130 |
| ... with the centre at its containment line (± 0.5 mm) | **158 / 165** | **126 / 127** |
| axial peak | 12.0 / 12.0 | 9.0 / 12.0 |

The copy op's samples on its line caused the pick run's 200.48 µm
deflection and the export refusal (§10.7). Op 1's other deep samples
(about 460–500) are not on its line. Most are the unplanned free-edge
strip of §10.8, where a later pass meets a full column. That is not
G-CLIPAIR.

### 11.1b Reconciled with D9's peaks (P15 "D23 addendum", `fix13/peaks_d23.json`)

`d23_why/peak_move.py` and `op1_deep.py` (read only) give the answers.

**Copy op: my 126 / 127 are D9's peak move.** Of my deep samples at the
copy op's containment line, 125 lie on one move: headon 1421, pick 1420.
- The move is a straight cut, 57 mm long, at x 151.06 from y −71.2 to
  −14.3, at Z 2.01.
- Its centre is 3.2 mm outside the silhouette (the copy op's line is at
  3.0) and 2.02 mm inside the stock's +X edge.
- Its axial peak is 9.0 mm (headon) and 12.0 mm (pick).

So D9's place ("beyond the mesh bbox, 2 mm inside the stock edge") and
mine ("at the containment line") are the same samples. On that side the
line lies beyond the mesh bbox.

**Why the cut is deep: both items act, in sequence.**
1. **The stock stands because of the free edge (§10.8).** Op 1 cut within
   R of the move's path only at Z ≥ 17 (17.0–18.5, the Z 17 level and
   its drape). Below Z 17 op 1 never came there. On that side its strip
   is about 2.2 mm wide in centre space (wall + 3 to the stock edge), and
   C31 gives it no centre region.
2. **The copy op meets it in one level because its planner cannot see it
   (G-CLIPAIR).**
   - The copy op cut within its R of the path at every level (17.0,
     14.01, 11.01, 8.01, 5.01, 2.01), so the columns at and inside its
     line were taken level by level.
   - The 9–12 mm the move meets lies beyond its line: x 150.9 to 152.56
     is the disc reach past the line, inside the stock box. The O5b clear
     holds that band as air in the copy op's planner stock, so no copy
     region cell targets it.
   - At Z 2.01 this long move, centred 0.2 mm outside the line, takes the
     whole standing band at once.

**What follows for P-CA1 / P-CA2:**
- Either fix alone should remove this move's deep cut.
  - §11 (c): the copy op's region gains the centres whose disc meets
    the band, so the band is cut one Depth/Pass per level.
  - The C31 free-edge item: op 1 takes the strip first, so the band is
    not there.
- P-CA1 and P-CA2 stand for §11 as designed. To keep them falsifiable,
  §11 must be measured BEFORE the C31 free-edge item lands, or with it
  absent. Otherwise the C31 item could pass them alone.
- P-CA1, restated with the move: the deep samples of the copy op's
  long containment-line cut (headon move 1421 / pick 1420 or its
  successor at the same place) → 0. The copy op's axial peak ≤ 4.5 mm.

**Op 1: the 12 mm samples are the slot's, not G-CLIPAIR's.**
`op1_deep.py`, op-1 samples with axial engagement over 9.0 mm:

| Run | over 9.0 | ... within ± 0.5 mm of op 1's line | wall distance p5 / p50 / p95 | levels |
|---|---|---|---|---|
| C38a (nibble, no slot) | 89 (peak 18.0) | 22 | 4.0 / 5.2 / 5.7 | Z 11, 5 |
| D23 pick (slot) | 627 (peak 12.0) | 165 | 4.4 / 5.4 / 5.9 | Z 11, 5 |
| D23 headon (slot) | 654 (peak 12.0) | 158 | — | Z 11, 5 |

The D23 samples sit at the corridor's outer edge (median 5.2–5.4 mm
from the wall), where the corridor loops ran. That is D9's reading: the
strip openings met the strip's column at full height (23 − 11 = 12). My
§11.1 row "158 / 165 at the containment line" counted those slot samples
under G-CLIPAIR, which was wrong.

With the slot removed (D24), op 1 returns to about the nibble's state:
89 over 9.0 mm, 22 of them within ± 0.5 mm of the line.
- **P-CA3, restated:** against the D24 run, not D23. Op 1's samples over
  9.0 mm within ± 0.5 mm of its containment line fall to ≤ 10 % of their
  D24 count (C38a, as a stand-in, has 22). If D24 has fewer than 10
  there, P-CA3 is vacuous, and I report it so.
- Op 1's other deep samples are the strip opening (C38a: peak 18 mm at Z
  11 / 5, 89 in all). They are a strip-opening effect, not G-CLIPAIR, and
  §11 is not predicted to change them by more than 10 %.

### 11.2 The fix (research arm only; shipped byte-identical)

The boundary clear does two jobs, which the fix separates:

**(a) Keep the reachable stock in the planner.** The clear empties only
the cells outside `boundary ⊕ R`, the boundary grown by the tool radius.
No disc with its centre inside the boundary reaches those cells, so
clearing them changes nothing that a cut can meet. The cells between the
boundary and `boundary ⊕ R` keep their real stock. Every planner read
(the stamp, `plan_entry`, the keep-down link test, the lift's air test,
C22a's held-link test) then sees the stock that the cutter meets.

**(b) Keep the region inside the boundary.** O5b needs regions (in
tool-centre space) to respect the boundary, so that no centre is planned
outside it. In `build_material_bool_grid` and `level_gate`, a cell whose
centre is outside the boundary is not a region cell for the research arm,
whatever its stock. Today the clear gives this for free; with (a) it must
be explicit.

**(c) Plan the centres that can cut the band.** A region cell is a
CENTRE cell, but its material test reads the stock of its own column only.
So a centre inside the boundary whose own column is clear is not a region
cell, even when its disc reaches stock standing beyond the boundary. That
is why the copy op never cut its band level by level, and then met 9–12 mm
of it at one level.

For the research arm, a cell inside the boundary is also a region cell
when some cell of the band (outside the boundary, inside `boundary ⊕ R`,
with stock above the level floor) lies within R of it. Implementation:
1. the band mask (cells outside the boundary with material above the
   level floor, and outside the band cleared by (a));
2. dilated by a disc of R;
3. intersected with the boundary interior;
4. OR-ed into the region grid.

The 2D planner then plans those centres at each level, and the cutter
takes the band one Depth/Pass at a time, as it takes any wall.

**Where it goes:**
- `path.rs` at the boundary clear: a cfg(research) branch, used when the
  arm is the research arm, that clears outside `boundary ⊕ R` and keeps a
  boundary mask on the context;
- `clearing.rs` `build_material_bool_grid` / `level_gate`: the mask for
  (b) and the band dilation for (c), research only;
- the By Area region map (`region_map.rs`) reads the same cell test. Its
  research use needs the same mask, or the arm stays Global (it is Global
  on Corne and wanaka200).

**Shipped:** the shipped branch is unchanged (the clear as now, no mask,
no dilation), so byte parity holds by construction. No new constant: the
boundary, R, the planner cell and the level floor.

**Interactions:**
- The C31 grow already treats the band as stock (§5.1), so the 2D grid
  and the 3D planner stock now agree there.
- The stock-box clip (item B) is unchanged.
- C22a's held-link stock test gets the right answer at the containment
  line.

### 11.3 Pre-registration (Corne, both columns, against D23)

- P-CA1: the copy op's deep samples at its containment line: 126 / 127 →
  0. All its deep samples ≤ 10 (the 3 inside the silhouette may stay).
  Axial peak ≤ 4.5 mm (1.5 × Depth/Pass).
- P-CA2: the copy op's deflection: no tool-load export refusal in the
  pick column. The deflection peak falls below the refusal limit; I give
  no number for it before the run.
- P-CA3: op 1's deep samples at its containment line: 158 / 165 → ≤ 16
  (10 %). Op 1's other deep samples (the free-edge strip, §10.8) are not
  predicted to change by more than 10 %.
- P-CA4: the 6–9 mm band (op 1's containment band) does not get worse
  than D23 (126). The copy op's reach band 1.5–4.5 mm after the full job
  falls, because the copy op now cuts it level by level. The full-job
  number is not in the R4 method (after op 1 only), so I give the
  direction, not a bound.
- P-CA5: cost. More copy-op cut (the band centres, about the band length
  × levels) and more retracts where a keep-down link would cross the now
  visible band. Plunge-entry spans may rise. I bound the copy op's
  runtime at no more than D23 + 25 % (pick 1463.3 s → ≤ 1829 s).
- P-CA6: shipped neutrality: byte parity green without `research`, and
  the 13 adaptive3d sentries green.
- P-CA7: wanaka200 back rough uses `containment = "inside"` with offset 0.
  The band beyond its boundary is the terrain's own footprint, where the
  part stands above the stock cells. The fix adds region cells there only
  where stock stands above the level floor in the band. P: the wanaka200
  cycle time changes by less than 2 %, and no new hard bar appears.

Falsifier: if P-CA1 fails while (a) is in, the deep copy cuts are not the
planner's blind band, and the mechanism of §10.7 is wrong.

### 11.4 As built (2026-10-26; uncommitted in adaptive-head, rustfmt clean, not built)

The changes are research-only and apply only when the arm is `Geodesic`
AND the ordering is Global. By Area is unchanged: `boundary_reach` gives
`None`, and the shipped clear runs.
- **`adaptive3d/research_outer_band.rs`:**
  - `boundary_reach(params, boundary, R)`: the boundary grown by R
    (`offset_polygon(boundary, −R)`), or `None`.
  - `ResearchClip { inside, rows, cols, tool_radius, cell_size }` and
    `restrict_regions(grid, padded_rows, padded_cols)`. On the padded
    grid, cell `(r+1, c+1)` is stock cell `(r, c)`.
    - (b) It clears every material cell whose centre is outside the
      boundary, and collects those cells as the band.
    - (c) It sets every inside cell within R of a band cell (a disc of
      R, on the cell centres).
- **`adaptive3d/path.rs`, the boundary clear:** the cell test is now
  `inside = boundary.contains_point(p)`. With a reach, the research
  branch keeps a cell when it is inside the boundary OR inside the
  reach, and it records `inside` per cell. After the loop it builds the
  `ResearchClip`.
  - Shipped, `kept = inside`, the same predicate as before.
  - The ctx literal gains `research_clip`, after C22's
    `research_entry_causes`.
- **`adaptive3d/clearing.rs`:**
  - the `research_clip` ctx field;
  - in `detect_and_order_regions`, right after `build_material_bool_grid`,
    a research block that calls `restrict_regions` on the grid.
  - `level_gate` is unchanged. It counts the band as material, so a level
    whose only stock is the band is still planned, and (c) plans it from
    inside.
- **The border clear is not touched.** With `world_stock_xy_bbox` (the
  Corne projects pass it), the cells between the mesh bbox and the stock
  box keep their stock already (F-027). So the copy op's band at
  x 150.9–152.56 reaches the boundary clear, which now keeps it.

**D25 verify:**
- the D24 gates;
- byte parity plain and research (P-CA6);
- the 13 sentries;
- the Corne spiral_pick and headon runs and wanaka200, with the fix13
  flags (`RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug`,
  `--sim-artifact`);
- then `d23_why/peak_move.py`, `clipair_base.py` and `op1_deep.py` on
  the new samples (`extract.py` with the new run paths), against
  P-CA1–P-CA7 (§11.3 / §11.1b).

Read P-CA3 against D24's count.

## 12. D24 Corne: where the cost went, the peaks, the band, and whether G-b earns it (2026-10-26)

Read only. The scripts are in `phase3/c38/d24_why/`: `extract.py`
(with segment times), `reversals.py`, `descents.py`, `peaks.py`,
`c22a_drops.py`, `linkbar.py` and `assert_d24.py`. Each asserting script
prints `ASSERTS OK`.
- C35 is fix12, the base before C38.
- D24 is fix14: G-b, the stock-box clip, no slot, and C22a.
- Op 1 is "3D Rough 1". "The strip" means op-1 cutter centres 3–6.2 mm
  outside the silhouette.
- **Places come from the samples, in the model frame.** `program.nc` is
  in the work frame, so its X/Y/Z do not match the part map. A
  junction's place is the first sample of its second move.

### 12.1 The 1968 reversals are the nibble's cross-strip steps

`reversals.py` uses `fix11/audit.py`'s rule (a fed-to-fed XY junction
over 90°):

| Op 1 | C35 | D24 |
|---|---|---|
| planar reversals | 353 | **1968** |
| ... in the outer strip | 71 | **1675** |
| ... of those at Z 11 / 5 / 0.5 | 0 / 0 / 0 | **564 / 518 / 518** |
| ... at Z 17 | 61 | 65 |
| ... inside the silhouette | 280 | 274 |
| shorter of the two moves, median | 1.04 mm | 1.5 mm |
| op 1 sample time | 1451.7 s | 1871.7 s (+420.0) |

- **Where:** 1600 of the 1615 new reversals are in the strip at its
  three levels, the nibble's cross-strip steps (§9.1). The reversals
  inside the silhouette did not change (280 → 274).
- **Where the +420 s of op 1 goes** (`fix11/audit.py` time split, C35 →
  D24):
  - plunge feed +202.2 s;
  - cut +122.7 s;
  - lift feed +49.6 s;
  - fed in air +26.8 s;
  - rapid +18.5 s.
- **Of that:**
  - the strip's cut samples at the strip levels take 47.0 s;
  - the moves that meet at the strip's reversals take 91.9 s;
  - the descents in the strip rise from 165 to 403, and their time from
    49.9 s to 124.0 s (`descents.py`). All descents together rise from
    537.4 s to 633.8 s.
- **So the reversals are about 22 % of op 1's added time** (91.9 / 420).
  The larger part is plunge and lift feed. The nibble's groups are
  short, so it has many entries, and each descends through the strip.
- **The copy op** adds 228.4 s (1305.2 → 1533.6).
- **The sim has no acceleration model** (the audit says so), so the real
  cost of 1600 full reversals is larger than their sample time.

### 12.2 The peaks and the fed-link bar: all G-CLIPAIR, and §11 (D25) covers them

`peaks.py` and `linkbar.py`:

| | Op 1, 18.0 mm at Z 5 (8 samples) | Copy op, 15.56 mm | Op 1 fed link, move 6921 |
|---|---|---|---|
| place | 7 at x 62–131 on the −Y side, 1 at (39.9, −3.0) | (2.78, −75.21, Z 0.5) | x 149.7, y −78.7 to −76.4, Z 11 |
| centre wall distance | 5.0–5.77 mm (the op's line is at 6.0) | 2.8 mm (line 3.0) | 4.1–5.69 mm (line 6.0) |
| to the stock-box edge | 7 of 8 at 11–29 mm; 1 at 4.87 | 2.47 mm (−X edge) | 3.26 mm (+X edge) |
| same op's cuts within R before | at Z 17 and Z 11, for all 8 | at Z 11.01 and 2.01 only | — |

- **The op-1 18 mm peaks are not the free edge** (7 of 8 lie 11–29 mm
  from the stock edge).
  - Each centre is 0.2–1.0 mm inside op 1's line, and op 1 cut within R
    of it at Z 17 and at Z 11.
  - Yet the disc at Z 5 met 18 mm (23 − 5): a column that NO earlier
    level touched. A column within R of this centre but beyond R of
    every earlier centre lies on the far side, beyond the line.
  - That band (6–9 mm) the O5b clear holds as air, so no level planned
    a centre to take it.
- **The copy-op peak is the mirror of §11.1b** (x 151, the +X edge): the
  −X edge, 2.47 mm inside the stock, centre 0.2 mm inside its line. The
  free edge left the stock there (op 1 has no centre region), and
  G-CLIPAIR hid it from the copy op.
- **The fed link** runs 4.1–5.7 mm from the wall at Z 11, 3.26 mm inside
  the +X stock edge. It removes 10.5 mm. It is a 3D keep-down link (C22a
  counts 0 held links over stock on Corne). Its test reads the planner
  stock, and the part of its disc beyond the line (6–8.7 mm) is O5b air.
- **§11 as built (D25) covers all three.**
  - (a) keeps the band beyond the line, so the link test and the stamps
    see it.
  - (c) plans the centres whose disc meets it at every level, so no level
    meets the full column.
- **Prediction for D25** (from §11.3): the op-1 18 mm peak, the copy
  15.56 mm peak and the fed-link bar go. Where op 1 has no centre region
  (the free edge, §10.8), (c) gives the copy op centres and the copy op
  takes the band level by level.

### 12.3 Why the band rose to 999 (D23 455, C38a 794)

| Op 1 in the strip, at Z 11 / 5 / 0.5 | C38a (nibble, no C22a) | D24 (nibble + C22a) |
|---|---|---|
| clearing-cut length | 4547.7 mm | 3041.4 mm |
| removed volume | 23671.5 mm³ | 17340.0 mm³ |
| C22a groups dropped in the strip (both pick plans) | — | 204 by the first filter, **548 more** by the replay rounds (up to 3 rounds) |

- **C22a costs the strip.** The nibble's plan is many short cross-strip
  groups.
  1. The F-038 filter drops those under 15 mm.
  2. C22a's replay then reads the plan without them, so more steps refuse
     or split.
  3. The next round drops those too.

  So C22a removes a third of the nibble's strip cut (4548 → 3041 mm).
  The band rises from 794 (C38a) to 999.
- **D23's 455 was the slot,** which cut along the strip without groups
  (and unfed, §10.1).

### 12.4 Does G-b earn its cost on Corne? The trade-off

| Corne | 3–9 mm band | op 1 | copy op | cycle | note |
|---|---|---|---|---|---|
| C35 (F-038 skip, strip never planned) | 1951 | 1451.7 s | 1305.2 s | 2757 s | the base |
| D24 (G-b + nibble + C22a) | 999 | 1871.7 s | 1533.6 s | 3405 s | +648 s for −952 mm²; 1600 reversals; the bar is back |
| C38a (G-b + nibble, no C22a) | 794 | 1750.7 s | 1694.3 s | 3445 s | — |
| D23 pick (G-b + slot, UNFED) | 455 | 1468.4 s (≈ 1504 fed, §10.1) | 1463.3 s | 2932 s | the slot's rate: about −1496 mm² for about +52 s on op 1 |
| D20 contour_parallel (shipped CP) | 63 | — | — | 2022 s (whole job) | no pass load |

**My view: G-b with the nibble does not earn its cost.** It buys 952 mm²
for 648 s (0.68 s per mm²).
- 22 % of op 1's added time is the reversals.
- Most of the rest is entries, because the nibble is many short groups.
- C22a correctly re-reads the plan and drops a third of the nibble.

The band that matters is cut more cheaply along the strip than across
it.

**The two options, with the same physics:**

- **(A) G-b plus a FED slot.** The slot as in D23, with the merge defect
  of §10.1 fixed: match the feed by the distance of each emitted move to
  the slot chain, not by one recorded segment.
  - Measured: band 455, op 1 ≈ 1504 s (C35 + 52 s).
  - It plans the strip in the region machinery, so G-CLIPAIR (§11) and
    the stock-box clip apply.
  - The slot ruling exists.
  - Remaining: the free-edge 403 mm² (§10.8, C31).
- **(B) Keep the F-038 skip, and add a wall-following contour pass on the
  outer strip, as CP does.**
  - This is the same cut as (A): passes along the strip, the first a slot
    over the ceiling, which needs the same c/w feed under the ruling.
  - It runs outside the region machinery, like the waterline. It needs
    its own boundary (the strip between the wall locus and the
    containment line), its own entry and its own G-CLIPAIR handling.
  - Cost: about the same feed time as (A). About 445 mm × 3 levels × 1–2
    loops at 0.22–0.95 F; D23 says +36 s of feed on the slot.
  - It keeps the F-038 skip, which still skips every other narrow region
    the field cannot seed (on Corne, the copy op's regions).

**The trade between (A) and (B):**
- (A) reuses the C38 machinery (the gate, the judge, the clip, and §11)
  and needs one defect fixed (the feed match).
- (B) needs a new producer outside the regions, and it duplicates (A)'s
  geometry.
- Both leave the free-edge band to C31.
- **Against the nibble:** either option saves about 370 s on op 1
  (1871.7 against about 1504 s), removes about 1600 reversals, and leaves
  about half the band (455 against 999).

I pre-register nothing yet, as asked.

## 13. Design (A): G-b plus a FED slot along the strip (2026-10-26; for approval, no code until D26 reports)

### 13.1 The plan is the removed corridor opening, unchanged

The plan is the D23 `corridor_opening` (git `6aefca969`,
`research_outer_band.rs`):
- the gate: the 2D tool-centre region has no centre clearance of R;
- the loops of the tool-centre region at `k · s`;
- each step at most `s`, and each step a `Slot` in `SlicePlan::slots`.

It comes back as it was. Only the feed match (§13.2) and the records
(§13.3) are new. The replay admits slot steps as turns (each step is at
most `s` < `turn_bound`), as in D23. The D23 strip result (band 455,
2940 over-c samples at full feed) is the measured base of the plan.

### 13.2 The feed match: a move gets the slot feed by its distance to the slot chain

**The defect (§10.1).** `apply_slice_turn_feeds` lowers a `Linear` move
only when its start and its target lie within 1e-6 mm of ONE recorded
segment. The 3D RDP merges collinear slot steps, so a merged move spans
several steps and matches none.

**The match for slots** (a new function beside it, research only; turns
keep the exact match, which §10.9 shows works):

- **The chain.** The slot records of one loop, in plan order, form a
  polyline: the loop's chain. Each record keeps its level `z`, its loop
  id, and its feed factor `c / max(w, area)` from its measured
  engagement.
- **The tolerance.** `d = Adaptive3dParams::emission_chord_tolerance()`,
  the one RDP tolerance that simplified the slot `Cut` (research arm:
  capped at the load margin, C26). Nothing else moves a cut in XY between
  the plan and the toolpath:
  - the lift and the drape change Z only;
  - the boundary clip cuts a segment at a point on it.

  So `d` is derived: the RDP tolerance plus the 1e-6 mm same-point
  tolerance of the turn match. No typed number.
- **Why `d` holds after the merge (both ways).** RDP keeps original
  vertices `v_i`, `v_j` and drops the vertices between them only if each
  lies within `d` of the segment `v_i v_j`.
  - The tube of radius `d` round a segment is convex, so the whole
    sub-chain from `v_i` to `v_j` lies within `d` of the segment.
  - The sub-chain is continuous from `v_i` to `v_j`, so its projection
    onto the segment covers the segment. So every point of the merged
    segment lies within `d` of the chain.
  - In 3D the distance is in 3D; the XY distance is no larger.
- **The test for a move.** A `Linear` fed move `[a, b]` at level `z`
  (within the drape of the record) gets the slot feed when all four hold:
  1. `a`, `b` and the points of `[a, b]` at the planner cell's spacing
     all lie within `d` of ONE chain;
  2. the move runs the chain's way: the dot product of `b − a` with the
     chain's tangent at the projection of the move's midpoint is
     positive;
  3. each slot step that the move covers (its projection interval on the
     chain) is not yet consumed;
  4. the move is at the record's level.

  Its feed becomes `min(f, F · min factor over the covered steps)`, and
  the covered steps are marked consumed.
- **Why it cannot lower a non-slot move.**
  - Plan order: a piece's corridor plan puts its slot loops FIRST, and
    the cleanup's residue contours, wall pass and mop come after them
    in the same piece's plan.
  - The toolpath keeps the plan order (no rapid reorder, `adaptive/CLAUDE.md`).
  - So, at the slot's level, the slot's own moves are the first that
    satisfy (1) and (2), and they consume every step (3).
  - A later move that retraces the slot (the wall pass runs along the
    loop k = 0) finds its steps consumed and keeps its feed.
  - A move of an EARLIER piece or region of the same level could
    satisfy (1) only if its piece's tool-centre region comes within `d`
    of this piece's boundary. Pieces come from `inset_piece_outlines` and
    are disjoint, so this needs two pieces closer than `d` (0.3 mm or
    less) at a neck.
  - The match counts such cases (`matched_before_own`: a chain step
    whose first match is not in the run of moves that follows the
    loop's `Rapid`), and the test asserts 0. So no non-slot move is
    lowered, or the run says so.
- **It can only lower.** The feed is `min(f, …)`, as for turns. A slot
  step whose factor is 1 (it held the ceiling) leaves the feed as it is.
  Arc fitting runs after this in the dressups and keeps a feed change as
  a move boundary. The D23 and D24 sims show reduced turn feeds surviving
  the dressups.

### 13.3 The slot summary line and the heavy-test assert

- **The log line.** One `info` line per op in `adaptive3d/mod.rs`, where
  `apply_slice_turn_feeds` hands back `(turns, slots)`. C22's turn line is
  there already (D24); the slot line sits beside it:
  `C38 slots: loops, planned steps, cut, fed, unfed, matched_before_own, reduced, lowest factor, longest merged move`.
  `unfed` counts the steps with a factor under 1 that no move carries.
- **The heavy C12 test assert** (`tests/adaptive3d_geodesic_arm_c12.rs`,
  heavy mode):
  - It needs a corridor case. A straight channel between two walls,
    `2R + 2·(SIMPLIFY_INSET + one cell)` + `s` wide in material space,
    so the 2D centre region is about `s` wide and the seed does not fit.
  - It asserts that the slot summary has `planned > 0` (non-vacuity),
    `unfed = 0` and `matched_before_own = 0`.
  - On the simulation, every sample with radial engagement over `c +`
    the discretisation tolerance, on a move within `d` of a slot chain at
    its level, runs at `F · factor` or below.
  - The test finds those moves independently, by the same distance rule
    on the printed chains. It does not use the feed, so the assert is not
    circular.

### 13.4 Does (A) need G-b?

**Yes, or an equivalent gate.** Without G-b the F-038 forecast runs. It
insets the region by `R + 0.25` and dry-runs the agent on the raw region,
which insets by R again. Both read 0 on a strip about 3 mm wide in centre
space, so the strip is skipped BEFORE any planner sees it, and the slot
never runs.

Two gates let it through:
- **G-b** (built, D22–D24 green): no forecast for the research arm, and
  the region is judged on its own plan. With the slot, the strip plans
  about 578 mm per level (D23), far over 15 mm, so it stays.
- **A forecast on the grown outline** (G-a, §3.1): the same answer for
  the strip, plus a second plan per region (the dry run) and a second
  code path.

**G-b is the simpler of the two.**

"F-038 skip plus the slot as the strip's planner" means planning the
strip outside the region machinery, which is option (B) of §12.4. It
would need its own boundary, entry and G-CLIPAIR handling. So (A) keeps
G-b.

**G-b's side effect is on the copy op.** Its sub-tool corridors open too.
D23 pick copy op: 1463.3 s, against C35's 1305.2 s, with 12 corridor
openings of 120–129 mm. §13.5 pre-registers that cost.

### 13.5 Pre-registration (Corne, spiral_pick, against D26 = D24 + G-CLIPAIR + C40)

D26 has not reported, so each bound below is relative to D26, or to a
measured D23/D24 anchor.

| Measure | Prediction for (A) on D26 | Anchor |
|---|---|---|
| 3–9 mm band | ≤ min(D26, 455) | D23's slot (455) without G-CLIPAIR; G-CLIPAIR only adds cuts |
| op 1 runtime | ≤ D26 op 1 − 300 s | the nibble against the fed slot: D24 1871.7 against D23 about 1504 (−368 s) |
| op-1 planar reversals in the outer strip at Z 11/5/0.5 | ≤ 10 % of D26's | D24: 1600 (the nibble). A loop reverses only at piece ends (D23: 4 pieces per level) |
| op-1 planar reversals inside the silhouette | D26 ± 10 % | D24 274, C35 280: the slot does not act there |
| op-1 descents in the outer strip (audit, by sample place) | ≤ 0.6 × D26's | D24 403, C35 165: one entry per loop, not one per nibble group |
| slot feed | over-c strip samples at full feed: 0 (D23: 2940); slot summary unfed 0, matched_before_own 0 | §10.1 |
| copy op runtime | ≤ D26 copy op + 10 % | D23 pick copy 1463.3 with slots, D24 1533.6 with the nibble |
| hard bars | no new bar against D26 (fed link, export refusal, collision) | — |
| wanaka200 | cycle D26 ± 2 %; openings as P-W1–W3 | §3.6, §10.6 |
| shipped | byte parity and the 13 sentries green | — |

**Falsifiers:**
- If the over-c strip samples at full feed are not 0, the match is wrong.
- If the band is over 455, the slot does not reach what it reached in
  D23. Read §10.3: the free-edge part stays with C31.
- If the op 1 runtime does not fall by 300 s, the nibble's cost was not
  the strip's planner.

### 13.6 Housekeeping

The sample `.npz` files under `phase3/c38/*_why/` are regenerated by each
folder's `extract.py`. They stay out of the commit set from now on. Each
analysis folder's scripts name the `simulation.json` they read.

### 13.7 Addition (lead, from C22 §14.6): clip the grown outline to the centre clip too

**The problem (C22 §14.6, wanaka200).** The C31 grown outline reaches past
the tool-centre containment. So the 2D plan's turns and held links lie
where the 3D stage may not emit, and the boundary clip (`clip_segment`,
`CentreClip`) drops them. The record shows this:
- D24: 74 turns cut, 72 of them "unfed";
- 66 held-link counter points lie outside the emitted toolpath's box
  (x 219.3 / 20.8, y 25.874 / 223.874, against the box 22.76–217.24 ×
  27.76–221.74).

**The fix (research arm, at the same call site as item B).** After the
stock-box clip, the grown outlines are clipped to the centre clip's reach:
- the reach is each `CentreClip` polygon grown by `R − m`, with `m =
  PERIMETER_INSET_MARGIN_MM`, the margin C31 already grows by.
- Why `R − m`: the 2D planner insets its outline by R, so the 2D
  tool-centre region becomes `(G ∩ (C ⊕ (R − m))) ⊖ R`, which lies within
  `C ⊖ m`. At a concave notch of `C` narrower than `2(R − m)`, the closing
  can leave up to the notch's depth.
- The result: every turn, held link and wrap end of the plan stands at
  least `m` inside the clip, as the sweep did (O5b).
- As for the box: a grown polygon wholly inside the reach is returned
  unchanged; an empty result keeps the shipped path. No new constant: R,
  `m`, and the clip's own polygons (already shrunk by its
  `EDGE_MARGIN_MM`).

**Code places (to agree with C22):**
- `clearing.rs`: the `research_grown_outlines(..)` call, one more
  `.and_then(..)` after the box clip.
- `centre_clip.rs`: one `#[cfg(feature = "research")]` accessor
  `polygons(&self) -> &[Polygon2]`. It is a shipped file, and the cfg
  keeps the shipped build free of a dead function.
- `research_outer_band.rs`: `clip_to_centre_reach(grown, clip, R, m)`.

C22 adds the record side: a turn is counted as cut only if its segment
survives the clip, with a `clipped` count and a proof line per unfed
turn.

**Pre-registration (against D26, with C22's record change in the same
build):**
- **wanaka200:**
  - turns "unfed" 72 → ≤ 2;
  - turns "clipped" (C22's new count) ≤ 2, because no turn is planned
    beyond the clip;
  - held-link counter points 66 → 0;
  - C40 `Clip`-cause entries at Z 19.54 / 14.08 / 8.62 → ≤ 10 % of
    D26's;
  - no cutting sample outside the D24 toolpath box, as before;
  - cycle time within ± 2 % of D26.
- **Corne:** here the plan already stands inside the clip. On Corne the
  region grid is inside the boundary (§11 b), and the turns are at inner
  walls (C22: fed 132/132, 230/230). So:
  - the turn summary has the same counts as D26 (cut = fed, unfed 0,
    clipped 0);
  - the held-link counter stays 0;
  - op 1 and the copy op's move counts are within ± 1 % of D26's.
- **Falsifier:** if wanaka's unfed count stays near 72 with the clip in,
  the turns are not lost to the clip, and C22's reading in §14.6 is wrong.

### 13.8 Which parts are generic (operator ruling 2026-10-26), and the interface each would offer

Ruling: "a keep down link should be generic and not built just for this.
same for anything else that could be useful from this research." For this
build the gating stays research-only. Below is the target placement for
adoption.
- None of these parts depends on the geodesic field. Each depends only on
  the planner stock, the boundary and the emitted path.
- G-CPRAPID and G-LINKPLOUGH (contour_parallel: rapids and keep-down
  links through stock, Corne and wanaka200) are the same planner-stock
  blindness, seen from the shipped default strategy.

| Part | Today (research) | Shared layer for adoption | Interface another strategy would call |
|---|---|---|---|
| **G-CLIPAIR (a): the stock model keeps what a disc can reach** | the branch in the `path.rs` boundary clear, gated by `boundary_reach` | the planner-stock set-up of `adaptive3d` (`path.rs`, the border and boundary clears). Every Adaptive3d strategy builds its stock there, contour_parallel included | `boundary_reach(boundary, R) -> Vec<Polygon2>` (the clear keeps `boundary ⊕ R`), with no strategy test. Then every strategy's stamps, `plan_entry`, keep-down link test and air test read the stock the cutter meets. That is the planner side of G-LINKPLOUGH / G-CPRAPID |
| **G-CLIPAIR (b, c): regions inside the boundary, plus the centres that reach the band** | `ResearchClip::restrict_regions`, called in `detect_and_order_regions` | the level grid (`clearing.rs`, `build_material_bool_grid` / `level_gate`). Contour_parallel builds its grid with the same function (`clear_z_level_contour_parallel`). Better, `geometry/region_mask.rs` beside the existing masks | `RegionReach { inside: Vec<bool>, rows, cols, cell, reach }` with `restrict(grid)`, applied by the grid builder for every strategy. `AreaMask` (By Area) would intersect with it, so By Area needs no separate path |
| **Stock-box clip of a grown outline** | `clip_to_stock_box` | `geometry/` (a polygon against the stock's XY box): any producer that grows an outline past a wall (C31 today; a CP-style offset from a boundary, tomorrow) | `clip_to_box(polys, BoundingBox2) -> Option<Vec<Polygon2>>`, unchanged inside the box |
| **Centre-clip reach of a grown outline** (§13.7) | `clip_to_centre_reach` plus a `#[cfg(research)]` `CentreClip::polygons()` | `centre_clip.rs` itself: the clip is shared, and so is "where may a centre plan". The accessor stops being research-only | `CentreClip::reach(margin) -> Vec<Polygon2>` (its polygons grown by `R − margin`), and `clip_to(polys, &reach)`. A strategy plans its 2D domain inside the reach, so nothing it plans is dropped by the clip later. That is the cause of C22's "cut, unfed" turns, and of any planned-then-clipped pass |
| **Fed-slot feed match** (§13.2) | a slot-only function beside `apply_slice_turn_feeds` | feed application on the emitted `Toolpath`: `dressup/feed_modulation.rs` or a small `toolpath` util, beside the turn feed of C22 | `apply_chain_feeds(tp, chains: &[FeedChain], tolerance) -> ChainFeedSummary`. Here `FeedChain { pts, level_z, factor_per_step }` and `tolerance` = the RDP tolerance that simplified the path. Any producer that knows a stretch must run slower (a slot, a turn, a full-width plunge-out, a CP first offset that slots) hands in its chains, and gets the counts back (fed, unfed, matched before own). C22's exact turn match becomes the special case of a chain of one step |
| **G-b judge and `RecordMark`** | `planned_cut_mm`, the judge in `plan_and_emit_region` | the region loop of the slice path. It is generic to every 2D-slice arm (AgentSearch and ContourSpiral too): "judge a region on its plan, not on a double-inset forecast" (§4 item 2) | `judge_region(plan, threshold) -> bool` in the region loop, used by the shipped arms once the double inset is gone. `RecordMark` stays research-only: it guards research records |

**What stays research-only:**
- the slot plan (`corridor_opening`): it plans with counted over-ceiling
  steps under an operator ruling made for this research;
- the geodesic-specific records.

**The order of adoption I would propose:**
1. **G-CLIPAIR (a)+(b,c), for every Adaptive3d strategy.** It fixes a
   planner-stock blindness that contour_parallel has too.
2. **The centre-clip reach.** It ends the planned-then-clipped waste.
3. **The chain feed match, in feed application.**
4. **The G-b judge for the shipped slice arms**, with the shipped fix of
   §4.

Each changes shipped output, so each needs its own byte-parity re-bless
on a measured cause. The interfaces above let me write the research build
so that adoption is a move of code and a removal of a `cfg`, not a
rewrite. In this build:
- `boundary_reach` and `ResearchClip` take only geometry (the strategy
  test sits at the call site);
- the chain feed match takes `FeedChain`s, not `SliceTurns`.

### 13.9 As built (2026-10-26, on 2c32340ab; uncommitted, rustfmt clean, not compiled)

All of it is research only; the shipped paths are unchanged.

**`adaptive3d/research_outer_band.rs`:**
- `corridor_opening` and `closed_steps` are restored from `6aefca969`
  unchanged.
- `boundary_reach(boundary, R)` is geometry only. The research arm and
  Global test moved to its call in `path.rs` (§13.8).
- `clip_to_centre_reach(grown, Option<&CentreClip>, R, m)` (§13.7).
- `FeedChain { pts, factors }`, `slot_chains(&SliceTurns)`,
  `apply_chain_feeds(tp, chains, tolerance, feed_rate)` and
  `ChainFeedSummary`.
  - `slot_chains` keeps the slot records whose ends are not both outside
    the clip, and splits them into chains where a record does not start
    at the end of the previous one.
  - `apply_chain_feeds` is §13.2. Its four tests:
    1. every point of the move at the tolerance spacing within `d` of
       one chain;
    2. the chain's way;
    3. an uncovered step;
    4. chains taken in plan order (a monotone cursor).

    Here `d` = `emission_chord_tolerance()` + 1e-6. It only lowers a feed.
  - The summary counts:
    - chains, steps, reduced, fed and unfed;
    - moves lowered;
    - `matched_out_of_run`: a match after a cutting move that is not on
      the chain, inside the chain's run. This replaces §13.2's
      `matched_before_own`, which the toolpath data cannot tell apart:
      no record gives a move its piece.
    - the lowest factor and the longest lowered move;
    - the chain list and the tolerance, for the test.

**`adaptive3d/clearing.rs`:**
- `research_slice_segments`: `.or_else(corridor_opening)`.
- The `research_grown_outlines` call: `.and_then(clip_to_centre_reach(..,
  ctx.centre_clip, ctx.tool_radius, PERIMETER_INSET_MARGIN_MM))` after the
  box clip.

**`adaptive3d/centre_clip.rs`:** the `#[cfg(feature = "research")]`
accessor `polygons()`.

**`adaptive3d/path.rs`:** the strategy test at the `boundary_reach` call.

**`adaptive3d/mod.rs` (C22's `research_turn_feeds`, as agreed):**
- a new parameter `chord_tolerance`, passed
  `params.emission_chord_tolerance()` at its one call;
- `slot_chains` + `apply_chain_feeds` before C22's
  `apply_slice_turn_feeds`;
- the info line `C38 slot chains`;
- the `Research3d.slot_chains` field;
- the re-exports `ChainFeedSummary` and `FeedChain`. `Research3d` is
  public, so its field type must be reachable.

**`tests/adaptive3d_geodesic_arm_c12.rs`:** a new heavy (ignored) test,
`the_corridor_slots_run_at_the_reduced_feed`.
- The fixture is a straight channel `2R + 2·(SIMPLIFY_INSET + cell) + s`
  = 9.6 mm wide and 60 mm long, at two levels.
- It asserts:
  - chains > 0 and reduced > 0 (non-vacuity);
  - unfed = 0 and matched_out_of_run = 0;
  - on the simulation, no sample over the screen bound at the full feed
    on a move that lies within the tolerance of a chain. The test finds
    those moves by its own distance code, not by the feed.

**D27 commands, beside C22's:**
- byte parity plain and research;
- the 13 adaptive3d sentries, which include the shipped `centre_clip`
  sentry `adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom`;
- `--test adaptive3d_geodesic_arm_c12` plain, and with
  `-- --ignored the_corridor_slots_run_at_the_reduced_feed`;
- both clippy configurations;
- the Corne spiral_pick and wanaka200 fixture runs with the fix13 flags,
  read against §13.5 and §13.7. Grep the `C38 slot chains` and
  `C22 turn summary` lines.

## 14. D26: the two fed-link bars are held links that cross the unplanned band; the counter reads the centre line only (2026-10-26)

Read only. `phase3/c38/d26_why/links.py` (`python3 -I`, prints `ASSERTS
OK`) reads the D26 spiral_pick simulation, which `extract.py` writes to
`samples_d26pick.npz`. The code citations are at `2c32340ab` plus the
D27 tree, which does not change these lines.

### 14.1 What the two moves cut

| | op 1, move 5565 (D24 6921: same XY, same Z) | copy op, move 6340 |
|---|---|---|
| samples, intent | 9, `linking`, at Z 11.0 | 3, `linking`, at Z 0.5 |
| centre wall distance | 4.10 → 5.69 mm (op 1's line is at 6) | 2.95 – 3.05 mm (ON the copy op's line, 3.0) |
| where it removes | ONLY the last sample: 6.037 mm³, axial 10.5 mm, radial 0.144. The first sample 1.73 mm³ / 0.31 mm; samples 2–8 nothing | samples 2–3: 0.333 + 0.852 mm³, axial 5.52 mm, radial 0 |
| what follows | clearing cuts (moves 5566 on) | clearing cuts |

**Both are thin, tall columns, not bulk stock.**
- Op 1 removes 6 mm³ at a height of 10.5 mm, an area of about 0.57 mm²
  (about 6 sim cells of 0.09 mm²).
- The copy op removes about 1.2 mm³ at 5.5 mm.

**Both lie off the link's centre line.**
- Op 1: a side contact of 0.86 mm (radial 0.144 × D) at the end, where
  the next cut starts. The centre is 5.69 mm from the wall, so the
  material lies 6–9 mm out: op 1's band beyond its line. D25 still has
  379 mm² standing there (P-CA4 failed).
- Copy op: its centre runs on its line. The columns are inside its disc
  but not under the centre, in the copy op's own band beyond 3 mm. Op 1
  left that band, and the copy op never cut it level by level.

### 14.2 The answers

1. **Not made by the boundary clip.** `clip_segment`
   (`centre_clip.rs:335-390`) never creates a `Link`.
   - Between clipped pieces of one cut it emits a `Rapid` (an entry),
     and that goes through `plan_entry` and gets an entry span.
   - It passes a `Link` through only when the chord is inside the clip
     and there is no gap.
   - Both moves have no entry span. So they are 2D links emitted
     keep-down (the C22/C26 `HeldLinks` route), which the clip let
     through.
2. **The planner stock does not necessarily see air.** The counter that
   reads 0 (`LevelSink::check_held_link`, `clearing.rs:2231-2270`) samples
   `sample_stock_top_at` along the link's CENTRE LINE only, at
   cell spacing. Both columns lie off the centre line, inside the disc.
   - So the counter cannot see them, and its 0 says only that the centre
     line is clear.
   - The keep-down proof of a 3D entry reads more: `plan_entry` uses
     `corridor_ceiling_z`, the conservative stock top on the SWEPT DISC of
     radius R. Held links do not go through that proof.
3. **Not beyond `boundary ⊕ R`, and not the border clear.**
   - The material lies 6–8.7 mm from the wall (op 1, reach 9) and 3–4.5
     mm (copy op, reach 4.5): inside the G-CLIPAIR reach, so the planner
     stock keeps it.
   - `compute/execute/rough_3d.rs` passes `world_stock_xy_bbox:
     Some(inputs.world_stock_xy)`, so the border clear keeps every cell
     inside the stock box.
4. **Why the stock stands there at all.** It is the band beyond the
   containment line that no centre reaches.
   - G-CLIPAIR (c) adds region cells for the centres at the line, but the
     C31 smoothing (0.3) and margin (one cell) trim the 2D centre region
     back by about 0.8 mm.
   - So the disc reaches only 8.2 of 9 mm (op 1) and about 4.2 of 4.5 mm
     (copy op).
   - The sliver that remains is what these links graze. It is the same C31
     margin item as §10.8, at a containment line in place of a stock
     edge.

### 14.3 The fix

1. **(The bar) One keep-down rule for every link.** This is the
   operator's ruling: "a keep down link should be generic".
   - A held link goes out keep-down only if the same proof that
     `plan_entry` writes passes: `corridor_ceiling_z(stock, from, to, R)` ≤
     the level + `stay_down_clearance_mm`. Otherwise it goes to the sink
     as an entry, and `plan_entry` decides.
   - The C22a counter then reads that same swept-disc proof, not the
     centre line.
   - That is C22's code (`plan_and_emit_region`'s held-link emit and
     `check_held_link`), and it is the shared interface of §13.8: one
     `keep_down_ok(stock, from, to, R, clearance)` for every producer.
   - P (Corne D26 inputs): both fed-link bars go. At most 2 held links
     become entries (op 1: 1, copy op: 1).
2. **(The cause) The band beyond the containment line**, with the free
   edge of §10.8, to C22 as the C31 margin item. At a CLIP edge (the
   containment line) the staircase that the smoothing removes is not
   there. The clip line is analytic, so the grown outline there can keep
   the full `R − m` reach and the disc reaches `line + R − m`. P: the 6–9
   mm band (D25 379) falls to the `m`-wide strip, about `m / 3 × 820 ≈ 68`
   mm² plus the stock-map cell.
3. **The triage bar is right to fire.** A 10.5 mm column, however thin,
   is a side bite at full depth on a link fed at the clearing feed. I
   propose no change to the bar.

### 13.10 The D27 v3 slot-test red: the corridor plan had no pass marker (2026-10-26)

**The probe** (`the_corridor_slots_run_at_the_reduced_feed`, run once by
C12 with the lead's leave):
- C31 opened both channel levels: "grown, no sweep", measured margin
  0.55.
- 120 slots were planned, but **0 slot records** were kept. The toolpath
  had 59 moves and 8 entries.

This is case (c): the slots were planned, but no record came through.

**The cause** (code, `research_geodesic.rs`, `pass_load_readout`): the
readout classes a step as a turn or a slot only when its producer is
`Producer::Clearing`, which only a `PassEntry` marker sets
(`Producer::after`). The field's plans push that marker; my corridor plan
had none. So every slot step read as "unmarked", `slice_turn_records`
found no slot step, and no record or chain existed.

**The fix** (`research_outer_band.rs`, `corridor_opening`): before the
first loop's `Rapid`, push `Marker(PassEntry { pass_index: 1, entry_x,
entry_y })`, as the field does.

**The result** (the same test, after the fix):
- 120 slots planned, 116 slot records;
- 167 moves, 8 entries;
- 2 slot chains, 116 steps, 108 reduced, 116 fed, 0 unfed, 110 moves
  lowered, 0 out of run, lowest factor 0.302, longest lowered move
  1.978 mm;
- 444 samples on slot moves, 0 over the screen bound at the full feed;
- the test passes in 1.61 s.

Both clippy commands are clean (plain and `research,test-support`: rc 0,
0 warnings).

**Note for reading D23.** D23's corridor plans also had no marker. The
channel shows that the marker changes more than the records: 59 → 167
moves, because the replay and the cleanup treat an unmarked plan
differently. So D23's slot geometry (band 455, op 1 1468 s) came from the
unmarked variant. D27 measures the marked one.

## 15. D29 v4: why the slot feeds do not reach the cut, and why my chain count says "unfed" (2026-10-26)

Read only. `phase3/c38/d29_why/` holds `extract.py` (the a_only Corne
simulation), `strip_feed.py`, `longmoves.py` and `assert_d29.py` (prints
`ASSERTS OK`).

### 15.1 Two counts disagree, and the simulation says neither feed survives

| Op 1, Corne D29 a_only | Source | slot steps | fed | unfed |
|---|---|---|---|---|
| C22 summary, exact match, `kind="slots"` | the adaptive3d toolpath, before the dressups | 2561 | 2561 (reduced 1983) | 0 |
| C38 slot chains, my chain match | the same toolpath | 2561 | 444 | 1604 |
| the simulation of the exported program | after the dressups | — | — | see below |

On the simulated program, at the strip levels (Z 11 / 5 / 0.5, centres
3–6.2 mm outside the wall), there are **6939 clearing samples**:
- **6924 at F 4000**, 9 at 1519 and 6 at 750;
- **1895 read over c = 0.219, all at F 4000**;
- **1554 read over the oracle bound 0.369, all at F 4000**.

Their moves are 0.3–1.18 mm long (p10 / p90), the size of one slot step,
so these are the slot moves themselves, not merged moves.
- Over all of op 1's clearing cuts the simulated program has only three
  feeds: 4000, 750 and 1519. No slot factor feed (F × 0.17 … F × 1)
  survives.
- The copy op is commanded at F 2429, yet 15477 of its clearing samples
  run at 3137 and 8075 at 4000.

### 15.2 Cause 1: the feed modulation dressup rewrites the planner's reduced feeds (this is the over-load)

The CLI runs have `adaptive_feed_modulation: true`, `ConstrainedMax`
(`summary.json`). In `dressup/feed_modulation.rs` (`ConstrainedMax`, about
:969), `new_feed = max_safe_feed_for_move(engagement, band, ctx,
predicted_cap)` is computed from the simulated engagement and the
chipload band. The commanded feed is read only when the move has no
engagement.
- So a move that the planner lowered to `F × c/w`, because its width is
  over the load ceiling, is raised again to the chip-load limit. The
  machine ceiling, 4000, caps it.
- The chip load does not see the pass-load (force) rule that the slot
  ruling is about.

**This defeats every research reduced feed in a CLI run: slots and C22's
turns alike.** The C22 and C38 summaries are computed before the
dressups, so they cannot see it. The channel test passes because it reads
the adaptive3d output directly, with no dressup. D23's unfed slots (§10.1)
were very likely this as well: in D23 the merge was there too, but the
modulator alone would have raised the feeds.

**The over-load C22 asked about:** op 1 alone has 1554 strip samples over
the oracle bound at F 4000 (about 466 mm of path at 0.3 mm per sample).
- These are counted, ruled over-ceiling steps that run at full feed
  instead of `F × c/w`.
- The copy op has the same mechanism. Its clearing cuts run above their
  commanded 2429.

### 15.3 Cause 2 (the count only): my chain match has no level, so it crosses levels

- **Why it crosses levels:** on Corne the strip has the same XY at Z 11,
  5 and 0.5. Op 1 has 12 chains: 3 levels × 4 pieces.
- **What happens:** a later non-slot move at one level (a wall pass or
  waterline along the strip, up to 55.8 mm long) lies within `d` of the
  NEXT level's chain. That chain is still fresh, so the move is lowered
  (wrongly) and consumes it, and the cursor jumps ahead.
- **The result:** the next level's own slot moves find their steps
  consumed. The skipped chains count as "unfed", and some long non-slot
  moves are lowered.
- **The design said so, the build did not:** §13.2 rule 4 named the level
  ("at the record's level"). I built it as a plan-order cursor alone,
  because the records carry no Z (§13.9). On the channel each level's
  chain was matched before any retrace, so the test did not see it.
- **What it costs:** the 1604 "unfed" is this counting error. The exact
  match fed every one of those steps before the dressups.

### 15.4 The fix

1. **(The over-load, generic: a feed set by a load rule is a ceiling for
   every later stage.)**
   - The modulator must not raise a move above a feed that the planner
     set from a load rule. `new_feed = new_feed.min(commanded)` for
     those moves.
   - Generic form (§13.8): a per-move `feed_cap` (`Option<f64>`) on the
     toolpath. The planner writes it for each turn and slot move, and
     every feed dressup reads `min(own, feed_cap)`.
   - The cheapest research-only form: the research arm hands the
     modulator the lowered moves (indices or caps), as Stage 4 already
     hands it `planner_engagement` / `predicted_cap`. The modulator caps
     those moves at their commanded feed.
   - **Where:** `dressup/feed_modulation.rs`, which is shipped code. With
     no caps handed in, shipped output stays byte-identical, because no
     shipped planner writes a cap.
   - **P:** on Corne the 1554 over-bound strip samples at full feed → 0,
     and the strip samples carry `F × c/w` (0.17–1.0 of F).
2. **(The count) Give the chain match a level, and run it only where the
   exact match failed.**
   - `SliceTurns` gains `levels: Vec<f64>`, one per record, pushed by
     `add` with the slice's `z_level` (C22's struct and call: one more
     argument).
   - `FeedChain` gains `level_z`. A move matches a chain only when both
     ends lie in `[level_z, level_z + depth_per_pass)`. A level's moves
     drape at or above it and below the level over it.
   - The chain match runs AFTER C22's exact match, on the slot records
     the exact match left unfed (`TurnRecord::fed == false`). Where the
     merge does not join steps (Corne), it then has nothing to do and
     lowers nothing.
   - **P:** Corne "C38 slot chains" unfed 1604 → 0 (op 1), 2130 → 0
     (copy op); moves lowered by the chain match → only merged slot
     moves (0 on Corne); the channel test stays green.

### 15.5 The build and its pre-registration (2026-10-26)

Code in the `adaptive-head` worktree, not committed. Both fixes are
built as §15.4 says, with the generic shape of §13.8.

**Fix 1, the generic feed cap.**
- `toolpath::Move` gains `feed_cap: Option<f64>`. Every shipped
  constructor writes `None`.
- The modulator (`adaptive_feed_modulate_with_materials`) applies the cap
  last, after every arm (ConstrainedMax, BandMid, bandless), the chipload
  floor and the plunge guard. It only lowers. The binding is the new
  `BindingConstraint::FeedCap` (label `feed-cap`, load regime
  `ToolLimited`).
- `feedopt` applies the cap last, after the smoothing.
- `rg with_feed_rate` finds no other feed stage. The export writes the
  feed it is given.
- A stage that rebuilds moves keeps the cap: the arc fit and the segment
  merge take the cap as an equality term of the run key (as `intent`).
  The transform, the simulation copy and the surface relink copy it. The
  simulation hash ignores it, as it ignores `intent`.
- The planner writes the cap where a load rule lowers a feed: my chain
  match (factor under 1), and C22's exact match (factor under 1; C22's
  code, agreed edit).
- Sentry: `tests/a_feed_cap_binds_every_feed_stage_c38.rs` (ConstrainedMax,
  BandMid, bandless, feedopt: the capped move leaves at its cap, every
  other move is bit-identical to a run with no cap, and a cap above the
  feed changes nothing). Unit test `does_not_merge_across_feed_cap_change`
  in `dressup/condition.rs`.
- Other users: any strategy whose planner lowers a feed by a rule
  (2D adaptive turns, a slot in a pocket, a plunge-like step) can set the
  cap. No shipped planner sets it yet.

**Fix 2, the level in the chain match.**
- `TurnRecord` gains `level_z: Option<f64>`, set by `SliceTurns::add`
  from the slice level (C22's struct; agreed edit).
- The chain match now runs AFTER `apply_slice_turn_feeds`. It reads the
  slot summary and takes only the records with `fed == false`. A chain
  splits at a level change. A move matches a chain only when both its
  ends lie in `[level_z − 1e-6, level_z + depth_per_pass)`.
- A limit: a level ladder with extra levels has levels closer than
  `depth_per_pass`. A move of the extra level over a chain can then pass
  the level test. The plan-order cursor and the "fresh step" rule still
  hold, and the match runs only on records the exact match left unfed.
- New log lines: `C38 feed caps exact=… chain=…` per op, and one
  `C38 capped turn` line per reduced turn the exact match fed (from, to,
  level, factor), so a script can find the turn moves in the program.

**Pre-registration, for the next Corne run (D30, same arm, a_only, same
job file as D29).** Op 1 is `3D Rough 1`; the copy op is
`3D Rough 1 (copy)`, commanded at F 2429.
1. Op 1 strip samples (`d29_why/strip_feed.py` on the D30 simulation)
   over the oracle bound 0.369 at F ≥ 3999: **1554 → 0**. Over c = 0.219
   at F ≥ 3999: 1895 → 0.
2. "C38 slot chains": op 1 **unfed 1604 → 0**, copy op **2130 → 0**.
   The exact match feeds every slot step on Corne (2561 / 2571), so both
   ops print `loops=0`, `moves_lowered=0`. D29 lowered 165 and 320 moves
   (one 51.5 mm long) by cross-level matches; D30 lowers none.
3. "C38 feed caps": `chain=0` on both ops. `exact` is near the reduced
   record count: op 1 129 + 1983 = 2112, copy op 2452. It is not equal
   to it, because the emitter can split one record into two moves.
4. The copy op: no sample on a capped move runs above 2429. Its
   clearing samples at 3137 / 4000 that lie on capped moves go to the
   cap or below.
5. The turns (C22's are overwritten today as well): op 1 has 129
   reduced turns (factor 0.28 … 1). The D30 `C38 capped turn` lines give
   their XY. A script maps them to program moves (both ends within
   1e-3 mm, both runs have one geometry) and counts, per run, the
   samples on those moves with feed above `F × factor`.
   - **P:** D29 > 0 (every such sample is at the modulated feed, and the
     strip shows the modulator lifts these moves to 4000); D30 = 0.
   - The count of D29 samples is the "effect on the turn samples" the
     lead asked for. It is measured from D30's lines; it is not known
     before D30.
6. Op 1 and copy op cycle time go up (feeds are only lowered). No
   prediction of the size.
7. Shipped parity: with no cap set, no shipped output changes. The
   dressup sentries and `constrained_max_modulation_f039`,
   `plunge_guard_p3`, `chipload_boundary_g_chip_ulp` stay green.

**Channel test result (2026-10-26, after the build).** C22's exact match
now feeds all 116 slot records on the channel. The chain match gets no
unfed record, so it has 0 chains. The old anchor `chains > 0` went red
for this reason alone. The anchors now read every slot record:
- records > 0, and some factor < 1;
- C22's `unfed == 0`, and the chain match feeds every reduced record that
  the exact match left (here 0 of 0);
- the simulation check finds the slot moves from loops built from all
  slot records, not from the chain list.

It judges the same 444 samples as before: 0 are over the screen at the
full feed. The test is green. On this fixture the 3D merge joins no slot
steps, so the chain match is a fallback with no work to do. That is the
same as the Corne prediction (§15.5 item 2).

**Ruling (lead, 2026-10-26).** The lead accepts the change to the slot
test's anchors. "chains > 0" stated the order of the two matchers, not a
property of the cut. The new anchors cover every slot record. They keep
`chains.unfed == 0` and the simulation check (0 samples over the screen
at the full feed), so they are at least as strict. D30 is frozen with
these fixes and C22's width read; D9 runs it, with the dressup sentries
and the viz and cli tests.

### 15.6 D30: the 87 strip samples still at F 4000, and the slot length over 0.471 (2026-10-27)

Read only. `phase3/c38/d30_why/` holds `strip87.py`, `departure.py`,
`slot_over.py` and `assert_d30.py` (prints `ASSERTS OK`). They read
D30's op 1 (`fix21/a_only/corne`) with `fix21/feed_check.py`'s rules.
Op 1's commanded feed is F = 1518.75 (job file, line 129).

#### 15.6.1 What the 87 are: the steps out of each corridor plunge

The 87 samples lie on 19 moves, all at program F 4000.
- **86 samples, 18 moves: the first moves of a corridor loop out of its
  straight plunge.**
  - There are 4 places, one at each strip piece, each at the three strip
    levels Z 11 / 5 / 0.5.
  - At each place the program plunges at 750 (the peck descents), then
    runs 1 or 2 straight moves at F 4000. They start less than R = 3 mm
    from the plunge and end at most 4.09 mm from it.
  - The first move after them is a capped slot step (F 312–434).
  - The sim reads 0.89–0.95 on them. That is a full slot.
- **Why they have no cap:** the 2D readout (`pass_load_readout`) exempts
  every readout step that starts within R of a straight plunge
  (`departing`, `plunge_departure_exempt`).
  - An exempt step returns before the turn and slot classing, so it
    makes no slot record. With no record, no matcher writes a feed or a
    cap.
  - The modulator then lifts the step to the band limit, 4000.
  - A readout step can be up to `3 × cell` long, so the exemption reaches
    past R: here to 4.09 mm.
  - The emitter joins the collinear exempt steps into one move (move
    4314 is 2.74 mm long).
- **1 sample, move 5086:** a G3 arc 0.16 mm long at F 4000, between
  capped slot steps (721 and 1300), radial 0.370 against the bound
  0.369.
  - The arc fit made it from linear moves with no cap (equal caps are a
    run-key term), so the readout gave those steps factor 1, or no
    record.
  - It is 0.001 over the bound. It is not a cap that failed.
- **None of the 87 is:**
  - a slot record that the exact match missed: `unfed=0`, `chain=0`;
  - a corridor move that is a slot record with a cap the modulator broke:
    the program feed equals the cap on every capped move, §15.6.2;
  - the clip;
  - one of C22's width-split steps: all 87 are at the strip levels, 3–6
    mm outside the wall, on the corridor loops.

Item 1 of §15.5 (P = 0) fails by these 87. The cap does what it was built
to do. The departure exemption is the open item.

#### 15.6.2 The 344.7 mm of slot moves over 0.471

`fix21/load_split.py` classes a sample as a slot move when its feed is
under the base feed (4000) and it is not on a capped turn move.
- **340.4 mm are ruled slot steps at their reduced feed.**
  - Their feed is F × 0.167 … F × 0.95, inside the slot factor range
    (C22: lowest 0.167, factor < 1).
  - The program feed equals the sample feed on all of them, so no later
    stage raised them.
  - 337.5 mm lie in the strip, 3–6.2 mm outside the wall (336.9 mm of
    it at the strip levels), and 2.9 mm lie 0–3 mm outside.
  - Their feed against `F × c / w_sim` (c = 0.219): median −0.031,
    p90 +0.010. The planner's width reads at or over the sim's.
  - These are the counted slots of the operator's slot ruling (full
    width, fed at `F × c/w`). They belong to the slot exemption, not to
    the over-load P.
  - D29 had them too, at F 4000. `load_split` classed them as "near A's
    steps" and "other" then: 1003 → 563 mm and 207 → 136 mm, while slot
    moves rose 40 → 345 mm.
- **4.3 mm are not slots:** samples at the plunge rate 750 (descents,
  inside the part's silhouette). The class rule takes them because 750 is
  under the base feed. They belong to the descents.

#### 15.6.3 The fix (proposed; not built)

The plunge departure stays exempt from the ceiling: the planner must be
able to leave a straight plunge. It must not be exempt from the feed
rule.

1. **Generic: a departure is a counted slot.**
   - The readout reads each departing step as now (it is not judged
     against the ceiling, and `exempt_steps` counts it).
   - It also records the step with its reading, as a new record kind
     `TurnKind::Departure`, with the slot factor
     `turn_feed_factor(c, swept, area)`.
   - The exact match and then the chain match feed it and set its cap,
     as for slots.
   - The chain match covers the case where the emitter joins exempt
     steps into one move (move 4314).
   - Where: `research_geodesic.rs` (the readout and the record kind:
     C22's code) and `slot_chains`, which also takes `Departure`
     records.
   - Other users: every plan that enters by a straight plunge. That is
     the field plans of the research arm and the shipped 2D adaptive (the
     same exemption), in a later parity-checked step.
2. **Narrow alternative:** only the corridor's departing steps make slot
   records (`kind Slot`). This fixes Corne. It leaves the field plans'
   departures at the full feed.

I recommend option 1.

**P (Corne, D31):**
- The 86 departure samples at F ≥ 3999 → 0. The strip count over 0.369
  at F ≥ 3999 goes from 87 to at most 1 (move 5086, the 0.370 arc).
- The departures run at about F × 0.22 (about 334 mm/min). This is
  12 departures × about 4 mm, so op 1 takes about 8 s more for the
  corridor departures.
- A field-plan departure gets the same rule, so the "other" class over
  0.471 (135.5 mm) may fall. I do not predict that size.
