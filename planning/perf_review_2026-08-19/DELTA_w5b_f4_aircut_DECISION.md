# W5B-F4 — air-cut threshold recalibration: DECISION PACKAGE

Follow-up to `DELTA_sim_w5b_landing.md` §9 row W5B-F4. Branch `tech-debt-3`,
measured at `3e2041f3`; the lane's instrument landed as `b5f04331`.

**This package changes no shipped threshold.** It inventories every consumer,
measures every family under the new default, proposes numbers, and states
which currently-shipping verdicts each proposal flips. The numbers belong to
the user.

---

## 0. The question, stated precisely

Since `a4ff2a8c` `StampDispatch::Auto` resolves to `Swept`. Under the retired
per-stamp kernel the air-cut percentage was largely a function of the
**simulation cell size** rather than of the toolpath: on one unchanged
toolpath it read 0.31 % at `cs = 0.25` and 89.61 % at `cs = 1.0`, while swept
read 5.95 → 11.04 (`DELTA_sim_w5_s1_DECISION.md` §2, class (ii-b)). Every
shipped air-cut bar was fitted against that reading.

So the question per bar is not "is the number nicer now". It is: **under the
swept instrument, does the current value produce wrong verdicts** — false
alarms on clean geometry, or silence on genuinely air-heavy paths?

Recalibration must not be "raise every bar until wanaka is green". §5 states
for each proposed number which wanaka flags **survive** and why that is
correct.

---

## 1. Consumer inventory — every site that compares air-cut against a constant

Denominators are not interchangeable (`simulation_cut.rs:585-600`,
`MEASUREMENT_DOMAINS.md` LH-1): `air_cut_pct_of_cutting_time` excludes rapids
from the denominator and is always ≥ `air_cut_pct_of_total_runtime`.
**Every shipped bar reads the total-runtime measure.** No threshold anywhere
in the workspace reads the cutting-time measure or the legacy
`air_cut_percentage` alias.

### 1.a Per-toolpath bands — `OperationType::air_cut_high_threshold_pct`

`crates/rs_cam_core/src/compute/catalog.rs:465-490`.

| Arm | file:line | value | ops |
|---|---|---:|---|
| drill kinematics | `catalog.rs:475` | **`None`** (suppressed) | `Drill`, `AlignmentPinDrill` |
| sparse projection | `catalog.rs:479` | **97.0** | `ProjectCurve` |
| 3D finish family | `catalog.rs:482-483` | **30.0** | `DropCutter`, `Scallop`, `UnifiedFinish`, `Waterline`, `Pencil`, `HorizontalFinish`, `SteepShallow`, `RampFinish`, `SpiralFinish`, `RadialFinish` |
| 2.5D clearing + 3D rough | `catalog.rs:486` | **40.0** | `Pocket`, `Face`, `Adaptive`, `Rest`, `Zigzag`, `Adaptive3d` |
| 2D contour | `catalog.rs:488` | **40.0** | `Profile`, `Chamfer`, `Inlay`, `VCarve`, `Trace` |

Denominator: **total runtime**, stated in the doc comment at `catalog.rs:461-464`.

### 1.b The one gate that reads those bands

| Site | file:line | what |
|---|---|---|
| band read | `crates/rs_cam_core/src/session/compute.rs:4024` | `tc.operation.op_type().air_cut_high_threshold_pct()`; `None` → `continue` |
| abstention | `session/compute.rs:4031-4041` | `MeasurabilityReport` `AirCut` verdict; abstains → recorded, not compared |
| measurement | `session/compute.rs:4046` | `tp_summary.air_cut_pct_of_total_runtime()` |
| comparison | `session/compute.rs:4047` | `air_pct > threshold` |
| disabled-op skip | `session/compute.rs:4019` | `!tc.enabled` → `continue` |
| verdict emission | `session/compute.rs:3537-3566` | ONE `Verdict { kind: VerdictKind::AirCut }` carrying **all** offenders in `offender_toolpath_ids` |

### 1.c Triage — no threshold of its own

| Site | file:line | what |
|---|---|---|
| class-B actions | `crates/rs_cam_core/src/sim_triage.rs:308-343` | copies `inputs.diagnostics` verbatim; **contains no air-cut constant** |
| verdict → diagnostic | `crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:40-42, 78` | one `Diagnostic` per `Verdict`; `Scope::Toolpath` when exactly one offender, else `Scope::Project` |
| advisories | `sim_triage.rs:355-400` | built from `trace.hotspots`, id `project.air_cut_high`, **no percentage threshold** — a hotspot is a wasted-time ranking |

**Consequence, measured in §4.d:** the triage's air-cut action count can never
exceed **one**, because every offender folds into a single diagnostic.

### 1.d Project-level constants

| # | Surface | file:line | value | reads |
|---|---|---|---:|---|
| 1 | GUI diagnostics banner | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:244` | **20.0** | `ct.summary.air_cut_pct_of_total_runtime()` (`:227-234`) |
| 2 | MCP `get_project_diagnostics` verdict | `crates/rs_cam_viz/src/controller/events/compute.rs:1890` | **20.0** | `s.air_cut_pct_of_total_runtime()` (`:1878`) |
| 3 | CLI `project` verdict | `crates/rs_cam_cli/src/project.rs:493` | **40.0** | `diag.air_cut_pct_of_total_runtime` |

(1) and (2) are the *same* bar on two surfaces. (3) is a different number on
the same quantity — **the workspace ships 20 and 40 as the project bar
simultaneously**, and which one an operator sees depends only on whether they
opened the GUI or the CLI.

### 1.e Narration

| Site | file:line | value | reads |
|---|---|---:|---|
| `AIR_CUT_WARNING_PERCENT` | `crates/rs_cam_core/src/narrate.rs:38` | **50.0** | — |
| ⚠ / ℹ marker | `narrate.rs:1925` | compares to 50.0 | `air_pct_of_total` |

A **third** number on the same quantity, per toolpath, not op-kind-aware. The
D7 ruling (`narrate.rs:1898-1906`) already moved this marker onto the
total-runtime reading "like every other air-cut threshold in the workspace" —
it did not make it read the same *band*.

Also in narration, threshold-free: `narrate.rs:1987-1996` prints both
denominators; `narrate.rs:1918-1923` prints the `NOT MEASURED` abstention.

### 1.f The per-sample substrate (not a percentage bar)

| Site | file:line | value | what |
|---|---|---:|---|
| air-cut classification | `simulation_cut.rs:1195` (and `:877`) | `radial_woc_fraction < 0.02` | which samples' seconds become `air_cut_time_s` |
| low-engagement classification | `simulation_cut.rs:1198` (and `:879`) | `< 0.10` | `low_engagement_time_s` |

Every percentage in this document is built on `0.02`. It is **not** "removed
nothing" — a sample that removes material but whose measured radial width is
under 2 % of the tool is booked as air. That is the mechanism by which cell
size leaked into the old reading, and it is why `sim_measurability` exists.

### 1.g Sites that touch air-cut and compare against nothing

Listed so a future reader does not have to re-derive that they are not
consumers:

* Optimizer — `tool_load/optimize/context.rs:50-61` (`air_cut_fraction_of_total_runtime_from_trace`), `candidate.rs:481-493`, `mod.rs:238-249`. Reported on every candidate, **never thresholded, never ranked on**.
* CLI per-toolpath print — `rs_cam_cli/src/main.rs:324-338`. Prints both denominators, no verdict.
* MCP wire — `rs_cam_viz/src/app/mcp.rs:4867-4872, 5061-5066`. Publishes both, no bar.
* Legacy alias `air_cut_percentage` — `session/mod.rs:1092`, `cli/project.rs:177`, `viz/compute.rs:1898`. Equal to the total-runtime reading; **no threshold reads it**.
* Feed optimiser — `feedopt.rs:42, 160, 244`, `air_cut_threshold: 0.05`. An **engagement fraction**, not a percentage of runtime. Different quantity, different units; out of scope here and named only so the `rg 'air_cut'` hit is accounted for.
* Dressup — `dressup.rs:1904-1930` `filter_air_cuts`. Geometric air-move removal against the stock; no percentage.
* Build-info feature flag `air_cut_op_kind_aware` — `rs_cam_mcp/src/server.rs:723, 806`.

**Total: eight sites that compare air-cut against a constant** — four
thresholded band arms (`ProjectCurve` 97, 3D finish 30, 2.5D clearing 40, 2D
contour 40; the drill arm is `None` and compares nothing), three project
constants (GUI 20, MCP 20, CLI 40), and the narration marker (50). Five
distinct values — **20, 30, 40, 50, 97** — for one quantity, all against the
same denominator, none re-measured since the kernel flip.

---

## 2. Evidence: how it was gathered

Three sources, kept separate because their standing differs.

| Source | What it is | Standing |
|---|---|---|
| **Goldens** | `tests/fixtures/perf_golden_sim_metrics{,_3d}.json`, re-baselined at `9b4505be`; old values recorded in `DELTA_sim_w5b_landing.md` §2 | Paired old→new on the same fixture. Highest. |
| **wanaka 0D** | `DELTA_sim_w5b_landing.md` §6, `tests/swept_wanaka_ab_s1.rs`, cell 0.4, shipped default | A real project, a real stack. Only one project. |
| **W5B-F4 harness** | `tests/air_cut_family_calibration_w5bf4.rs`, landed `b5f04331`. 38 rows: every family, two cell sizes, run twice — once under the default (swept) and once under `RS_CAM_STAMP_DISPATCH=whole_path` (the old `Auto` resolution) | Fresh, paired, but **isolated fixtures** — see below. |

### 2.a The harness's isolation choice, and what it costs

Each harness row is **one operation alone in its own session against fresh
stock**. That makes the number a property of that operation's own emitted
motion against a known stock rather than of whatever the previous op left. It
also means a finish op is skimming a full stock envelope where a real stack
would have it following a rough. Two consequences:

* `[Waterline]` isolated reads **40.45** where the 3D golden records
  **54.26** — the golden's waterline runs *second*, after drop-cutter has
  taken the material it would otherwise cut. Not a contradiction; a different
  question.
* `[Pocket]`, `[Profile]` and `[DropCutter]` are **first** in their goldens,
  so they reproduce, and that is the harness's own validity check.

### 2.b Validity check — six exact reproductions

Numbers the harness did not author, reproduced to the printed digit:

| Fixture / op | cell | recorded | harness (swept) | recorded old | harness (`whole_path`) |
|---|---:|---:|---:|---:|---:|
| 2.5D golden `[Pocket]` | 1.0 | 10.5521 | **10.552** | 60.258 | **60.258** |
| 2.5D golden `[Profile]` | 1.0 | 5.6110 | **5.611** | 28.171 | **28.171** |
| 3D golden `[DropCutter]` | 0.5 | 34.2833 | **34.283** | 43.235 | **43.235** |

Both kernels, both directions. The instrument is measuring the thing the
goldens measure.

---

## 3. Evidence: the measured table

`air_cut_pct_of_total_runtime`. "old" = `RS_CAM_STAMP_DISPATCH=whole_path`,
"new" = shipped default (swept). Δ = spread across the two cell sizes; it is
the **instrument-stability** column, not a quality column.

### 3.a 2.5D clearing / 3D rough — band 40

| op | cell | old | new | old Δ | new Δ | source |
|---|---:|---:|---:|---:|---:|---|
| `Pocket` | 0.5 | 14.446 | **8.656** | | | harness |
| `Pocket` | 1.0 | 60.258 | **10.552** | **+45.8 pp** | **+1.9 pp** | harness / golden |
| `Zigzag` (isolated) | 0.5 | 11.618 | **10.342** | | | harness |
| `Zigzag` (isolated) | 1.0 | 73.211 | **14.316** | **+61.6 pp** | **+4.0 pp** | harness |
| `Zigzag` (3rd in golden) | 1.0 | 94.290 | **90.877** | — | — | golden |
| `Face` | 0.5 | 22.046 | **12.280** | | | harness |
| `Face` | 1.0 | 70.885 | **15.456** | **+48.8 pp** | **+3.2 pp** | harness |
| `Adaptive` | 0.5 | 28.910 | **35.478** | | | harness |
| `Adaptive` | 1.0 | 59.588 | **33.088** | **+30.7 pp** | **−2.4 pp** | harness |
| `Adaptive3d` tp1 (back rough) | 0.4 | — | **16.73** | — | — | wanaka 0D |
| `Adaptive3d` tp5 (front rough) | 0.4 | — | **50.77** | — | — | wanaka 0D |

`Rest` was **not measured** — it needs a two-op cascade with
`claims_reference = machined_stock`, which is a different fixture programme.
See §7.

### 3.b 2D contour — band 40

| op | cell | old | new | old Δ | new Δ | abstains? |
|---|---:|---:|---:|---:|---:|---|
| `Profile` | 0.5 | 4.957 | **4.878** | | | no |
| `Profile` | 1.0 | 28.171 | **5.611** | **+23.2 pp** | **+0.7 pp** | no |
| `Trace` | 0.5 | 4.328 | **6.912** | | | no |
| `Trace` | 1.0 | 5.998 | **7.844** | +1.7 pp | +0.9 pp | no |
| `Chamfer` | 0.5 | 18.315 | **17.110** | | | no |
| `Chamfer` | 1.0 | 22.920 | **20.361** | +4.6 pp | +3.3 pp | no |
| `VCarve` | 0.5 | 97.198 | **96.875** | | | **YES** |
| `VCarve` | 1.0 | 97.198 | **96.941** | 0.0 pp | +0.1 pp | **YES** |
| `Inlay` | 0.5 | 86.587 | **85.044** | | | **YES** |
| `Inlay` | 1.0 | 89.416 | **88.980** | +2.8 pp | +3.9 pp | **YES** |

`VCarve` under the old kernel read `air_cut_pct_of_cutting_time` = **100.000 %**
with `average_engagement` **exactly 0.0000** while removing 12 101 mm³ — the
"hard zero dressed as a percent" pattern. Under swept it reads 99.667 % /
0.0007. **Both are non-readings, and `sim_measurability` says so**: the
`AirCut` metric abstains for `VCarve` and `Inlay` at both cell sizes, so the
40 band never sees them. Their percentages are in this table for completeness
and are **not** evidence about the band.

### 3.c 3D finish family — band 30

Hemisphere fixture, Ø6 ball nose (Ø3 for `Pencil`), no prior roughing pass.

| op | cell | old | new | old Δ | new Δ | fires 30 (new)? |
|---|---:|---:|---:|---:|---:|---|
| `DropCutter` | 0.25 | 24.637 | **34.123** | | | ✗ over |
| `DropCutter` | 0.50 | 43.235 | **34.283** | **+18.6 pp** | **+0.2 pp** | ✗ over |
| `Waterline` | 0.25 | 39.049 | **40.738** | | | ✗ over |
| `Waterline` | 0.50 | 39.031 | **40.454** | −0.0 pp | −0.3 pp | ✗ over |
| `Scallop` | 0.25 | 40.509 | **41.151** | | | ✗ over |
| `Scallop` | 0.50 | 43.367 | **41.612** | +2.9 pp | +0.5 pp | ✗ over |
| `UnifiedFinish` | 0.25 | 23.899 | **28.696** | | | ✓ under |
| `UnifiedFinish` | 0.50 | 33.101 | **27.782** | +9.2 pp | **−0.9 pp** | ✓ under |
| `SteepShallow` | 0.25 | 76.406 | **76.630** | | | ✗ over |
| `SteepShallow` | 0.50 | 78.080 | **78.091** | +1.7 pp | +1.5 pp | ✗ over |
| `RampFinish` | 0.25 | 41.641 | **42.082** | | | ✗ over |
| `RampFinish` | 0.50 | 43.731 | **42.476** | +2.1 pp | +0.4 pp | ✗ over |
| `SpiralFinish` | 0.25 | 39.058 | **40.757** | | | ✗ over |
| `SpiralFinish` | 0.50 | 39.581 | **40.526** | +0.5 pp | −0.2 pp | ✗ over |
| `RadialFinish` | 0.25 | 82.388 | **81.331** | | | ✗ over |
| `RadialFinish` | 0.50 | 84.729 | **82.332** | +2.3 pp | +1.0 pp | ✗ over |
| `HorizontalFinish` | 0.25 | 26.790 | **28.976** | | | ✓ under |
| `HorizontalFinish` | 0.50 | 36.562 | **36.697** | +9.8 pp | **+7.7 pp** | ✗ over |
| `Waterline` (2nd in golden) | 0.50 | 53.788 | **54.261** | — | — | ✗ over |
| `DropCutter` tp8 | 0.4 | — | **35.71** | — | — | ✗ over (wanaka) |
| `Pencil` tp9 | 0.4 | — | **55.83** | — | — | ✗ over (wanaka) |

`HorizontalFinish` was measured on a **flat plate**, not the hemisphere: on
terrain it emits nothing at all (`CLAUDE.md`: "useless on terrain"), so
measuring it there would have produced an absent row, not a low one.

**`Pencil` could not be measured on a synthetic fixture** — a smooth
hemisphere has no concave seams, so the op emits nothing. Its only post-flip
reading is wanaka tp9 = 55.83. **One data point.**

### 3.d Sparse projection — band 97

| op | cell | old | new | source |
|---|---:|---:|---:|---|
| `ProjectCurve` (isolated river) | 0.25 | 24.218 | **28.741** | harness |
| `ProjectCurve` (isolated river) | 0.50 | 41.446 | **13.175** | harness |
| `ProjectCurve` tp3 rivers | 0.4 | — | **15.97** | wanaka 0D |
| `ProjectCurve` tp4 lakes | 0.4 | — | **10.90** | wanaka 0D |

The band's stated rationale is *"Wanaka TPs read 78–92 % air-cut at-baseline.
Only flag near-total air (~97 %+)"* (`catalog.rs:476-479`). **Post-flip the
same project's project-curve ops read 15.97 and 10.90.** The 78–92 % the
band was fitted to was the artifact.

---

## 4. What did not reproduce, and one refutation

Reported rather than smoothed over.

### 4.a The golden's `[Zigzag]` 90.9 % is not evidence about zigzag

Measured isolated, the same op with the same dials reads **14.316** at the
same 1.0 mm cell. The golden's zigzag runs **third**, after pocket has
already removed the material it would cut, so 90.9 % is a property of the
fixture's *stacking*, not of the operation. Anyone using the golden's
`[Zigzag]` row to argue about the 2.5D band is reading a different question.
Same caution applies to the golden's `[Waterline]` 54.26 vs isolated 40.45.

### 4.b `ProjectCurve` is **not** cell-stable under swept

28.741 at cell 0.25 → **13.175** at cell 0.50: a **−15.6 pp** swing, and in
the *opposite* direction to the old kernel's +17.2 pp on the same fixture.
The swept instrument's stability claim, which holds within ±4 pp on 17 of the
19 op/fixture pairs measured here, **does not hold on this one**. The
population is small (878 / 846 samples, 91 moves), which is the likely cause
and is itself a reason not to build a tight band on it. Any `ProjectCurve`
number in §5 carries this caveat.

### 4.c `HorizontalFinish` is only weakly stabilised

+9.8 pp old → **+7.7 pp** new. Swept barely improves it. A Ø6 ball skimming a
plane is close to the lateral-resolution limit the `PERP_COVERAGE_GATE`
governs, so this is plausibly the abstention rule's neighbourhood rather than
a kernel property. Not investigated.

### 4.d **REFUTED** — the landing note's explanation of `triage_action_count: 3 → 1`

`DELTA_sim_w5b_landing.md` §2 states the 2.5D golden's action count fell
"because **two air-cut actions** dropped below their threshold". That cannot
happen: the air-cut verdict folds every offender into **one** `Verdict`
(`session/compute.rs:3552-3556`) and the adapter emits **one** `Diagnostic`
per `Verdict` (`from_project_diagnostics.rs:40-42`).

Reproduced on the golden's own fixture, both kernels, printed item by item
(`triage_action_composition_on_the_two_and_a_half_d_golden_fixture`):

| | old (`whole_path`) | new (swept) |
|---|---|---|
| actions | **3** | **1** |
| `project.crosses_standing_material` | **2** | **0** |
| `project.air_cut_high` | **1** — naming **`Pocket`** at 60 % | **1** — naming **`Zigzag`** at 91 % |
| `MeasurabilityAbstained` verdict | **yes** — `Zigzag`, 75 % of removing samples read zero engagement at 1.0 mm | none |
| advisories | 23 | 23 |

The counts 3 and 1 reproduce exactly. The **explanation** does not. What
actually dropped were **two `project.crosses_standing_material` actions** — a
different finding entirely (the R-12 standing-material measure, which reads
per-pass bite statistics that swept's sample-density-independent removal
changes). The air-cut action count was **one before and one after**; what
changed was its *subject*, from `Pocket` to `Zigzag`, because swept made
`Pocket` measurably fine and made `Zigzag` measurable at all.

**This is a correction for the programme ledger — number twelve.** It matters
beyond bookkeeping: a reader who believed air-cut actions were per-toolpath
would size the blast radius of a band change wrongly. The air-cut band can
move a project's action count by **at most one**.

---

## 5. Proposal

| # | Bar | file:line | current | **proposed** | flips? |
|---|---|---|---:|---:|---|
| P1 | 3D finish family | `catalog.rs:482-483` | 30.0 | **45.0** | yes — §5.1 |
| P2 | `ProjectCurve` | `catalog.rs:479` | 97.0 | **60.0** | none today |
| P3 | 2.5D clearing + 3D rough | `catalog.rs:486` | 40.0 | **40.0 — unchanged** | — |
| P4 | 2D contour | `catalog.rs:488` | 40.0 | **40.0 — unchanged** | — |
| P5 | drill / pin-drill | `catalog.rs:475` | `None` | **`None` — unchanged** | — |
| P6 | GUI banner + MCP verdict | `sim_diagnostics.rs:244`, `viz/compute.rs:1890` | 20.0 | **derive from the per-op offender list**; interim constant **40.0** | none |
| P7 | CLI verdict | `cli/project.rs:493` | 40.0 | **derive from the same list**; else unchanged | none |
| P8 | narration marker | `narrate.rs:38, 1925` | 50.0 | **read the op's own band**, fall back to 50.0 when `None` | none, given P1 |
| P9 | per-sample `< 0.02` | `simulation_cut.rs:1195` | 0.02 | **unchanged** | — |

### 5.1 P1 — 3D finish family 30 → 45

Nine 3D finish ops were measured on clean, defect-free geometry with the
correct tool. **Eight of the nine trip 30** at the golden's own resolution;
they also tripped it under the old kernel (9 of 9). A bar that fires on
essentially every well-formed instance of the family it governs is not
distinguishing anything — its stated meaning, ">30 % indicates poor boundary
or excess retraction" (`catalog.rs:480-481`), is not what it is measuring.
The defect-free cluster sits at **34.3–42.5** (`DropCutter` 34.3,
`SpiralFinish` 40.5, `Waterline` 40.5, `Scallop` 41.6, `RampFinish` 42.5),
with `UnifiedFinish` and `HorizontalFinish` below it. **45 clears that
cluster by ~2.5 pp and keeps the two genuine outliers — `SteepShallow` 78 and
`RadialFinish` 82 — flagged by a factor of 1.7–1.8.** Those two are
air-heavy by construction on a dome (radial spokes converge at the apex and
run out across flat margin; steep/shallow banding re-traverses its overlap),
which is exactly the "your strategy does not suit this surface" signal the
band should carry.

45 is a *judgement on a cluster of nine*, not a derived constant. If the user
prefers a bar that keeps `DropCutter` flagged, 36 is the alternative — it
clears only `DropCutter` and `UnifiedFinish` and leaves five clean ops
warning permanently. This package recommends 45 and says why below.

**Verdict flips under P1:**

| Verdict | now | under 45 | direction |
|---|---|---|---|
| 3D golden `[DropCutter]` 34.283 | fires | **green** | alarm removed |
| 3D golden `[Waterline]` 54.261 | fires | **fires** | unchanged |
| 3D golden `triage_action_count` | 3 | **3** (the air-cut action still fires, on `Waterline` alone) | unchanged — but the action's *message* changes from two offenders to one, so the golden's stored message text moves if it stores one |
| wanaka 0D tp8 `drop_cutter` 35.71 | fires | **green** | alarm removed |
| wanaka 0D tp9 `pencil` 55.83 | fires | **fires** | **survives** |
| wanaka 0D verdict headline | 3 offenders (tp5, tp8, tp9) | **2 offenders (tp5, tp9)** | narrowed |
| harness `Waterline` / `Scallop` / `RampFinish` / `SpiralFinish` / `HorizontalFinish@0.5` | fire | **green** | alarms removed |
| harness `SteepShallow` / `RadialFinish` | fire | **fire** | **survive** |

**Why wanaka tp9's flag surviving is correct.** Pencil detail at 55.83 % is
1.24× the proposed bar and 1.6× the defect-free finish cluster. On the wanaka
model the pencil pass is a Ø1 ball chasing valley seams across a 200 mm part;
it is the op the intra-region-link work exists for, and its air is
**count-bound** (retract trips), which is a fixable property of the path, not
an intrinsic one. Note it is a **single** data point — see §7 R3.

**Why wanaka tp8's flag being removed is a real cost, stated.** 35.71 % on a
`FromRemainingStock` drop-cutter finish is not obviously fine. But the same
op kind, on clean geometry, with no defect present, reads 34.1–34.3 in this
harness. **A bar at 30 cannot tell tp8 from a defect-free pass**, so it was
not giving that information anyway; it was giving a constant. Recovering the
tp8 signal needs a per-op *comparative* measure (tp8 vs the family's own
clean baseline), not a lower constant.

### 5.2 P2 — `ProjectCurve` 97 → 60

The 97 band's own rationale is a measurement that no longer reproduces: the
"wanaka TPs read 78–92 % at-baseline" the comment cites now read **15.97** and
**10.90** on the same project under the shipped kernel, and an isolated sparse
river reads 13.2–28.7. **97 is now unreachable — a dead gate**, and a dead
gate on a surface an agent reads is worse than no gate, because
`get_diagnostics` returning nothing for a project-curve op reads as
exoneration.

60 restores headroom (≈ 2× the highest reading measured) while leaving the
op's genuine sparseness unpunished. It is deliberately loose because of §4.b:
the `ProjectCurve` reading is the one that did **not** stabilise under swept,
and a tight band on an unstable measure is the defect this whole review keeps
finding.

**Verdict flips: none.** No project-curve reading anywhere in the repo
exceeds 60 today. This proposal buys future sensitivity, not a present
change, and the user is entitled to reject it on exactly that ground —
"leave 97, it fires on nothing either way" is a defensible alternative. The
argument against leaving it is that the *comment* would then be a citation to
a retired instrument, which is the failure mode `feedback_instrument_integrity`
names.

### 5.3 P3 — 2.5D clearing stays at 40

Swept readings: `Pocket` 8.7–10.6, `Zigzag` (isolated) 10.3–14.3, `Face`
12.3–15.5, `Adaptive` 33.1–35.5, `Adaptive3d` 16.7 / 50.8. The clean band is
**8.7–35.5**; 40 sits ~4.5 pp above the highest clean reading (`Adaptive`,
which is genuinely retract-heavy by construction).

* **Lower** — 35 would false-alarm on a clean `Adaptive` at both cell sizes.
* **Raise** — 55 would silence wanaka tp5 (50.77), the family's only genuine
  present-day catch.
* **Keep 40.** It is the only value in a narrow window, and it is already the
  shipped one.

**What survives:** wanaka tp5 `adaptive3d` 50.77 keeps firing while tp1 —
*the same op kind, the same project, the same tool* — reads 16.73 and stays
green. That 3× asymmetry inside one project is precisely what a working band
should surface, and it is the strongest single piece of evidence in this
package that 40 is currently right. The golden's stacked `[Zigzag]` at 90.9
also keeps firing, correctly: an op that traverses a region a prior op
already cleared *is* the thing the band is for.

### 5.4 P4 — 2D contour stays at 40, with a recorded dependency

Measurable readings: `Profile` 4.9–5.6, `Trace` 6.9–7.8, `Chamfer` 17.1–20.4.
The highest measurable reading in the family is `Chamfer` at 20.4, i.e. 19.6 pp
of headroom. Keeping 40 costs nothing and catches a genuinely broken contour.

**But the band's correctness for `VCarve` and `Inlay` rests entirely on
`sim_measurability` abstaining.** Their raw readings are 96.9 % and 85–89 %;
if the abstention rule ever narrows, both become instant permanent false
alarms on every project that contains them, with no code change to the
threshold at all. Two ways to hold that:

* **Recommended:** leave the band at 40 and add a sentry asserting that the
  `AirCut` metric abstains for a shipped `VCarve` fixture, so a change to the
  abstention rule lands as a red test rather than as a new warning on every
  sign-carving project. `b5f04331`'s harness already measures the abstention
  column; promoting it to an assertion is a one-line follow-up.
* **Rejected:** moving `VCarve`/`Inlay` into a sparse band (97-style). It
  would treat a **non-reading** as a high reading and thereby bless it. Their
  `average_engagement` is 0.0007 and 0.0126 while removing ~12 100 mm³ and
  ~12 300 mm³ respectively — the
  right answer is the abstention, not a band that accommodates the artifact.

### 5.5 P5 — drills stay `None`

Nothing in this wave touches Z-only kinematics, and `drill_summaries` /
`drill_gates` remain the actionable channel. Unchanged.

### 5.6 P6 / P7 — the project-level bar: 20 and 40 are both wrong, and so is any constant

Post-flip project readings, all three reference projects:

| Project | old | **new** | fires 20? | fires 40? |
|---|---:|---:|---|---|
| 2.5D golden | 74.81 | **50.03** | yes | yes |
| 3D golden | 52.27 | **51.38** | yes | yes |
| wanaka 0D | 55.06 | **44.09** | yes | yes |

**The GUI banner fires on every project the repo has, before and after the
flip.** A warning that is always on carries no information; it is the
"blanket project-wide threshold" the P1 RCA already argued against for the
per-op case (`session/compute.rs:3393-3397`) and then left standing at the
project level. And the workspace ships **two different constants** for the
same number — 20 in the GUI and MCP, 40 in the CLI — so the same project is
"WARNING" or "WARNING" for different reasons on different surfaces.

**Recommended:** derive the project verdict from the per-op offender list
that already exists (`air_cut_offenders_for_toolpaths`,
`session/compute.rs:3997`), i.e. *"N toolpaths over their op-kind band"* —
zero offenders, zero banner. That machinery is already computed for the
verdict list on the same call; the banner would stop being a fourth,
independent, un-calibrated number. Under P1+P3 the 3D golden and wanaka
would each show 1–2 offenders and stay red; the 2.5D golden would show 1
(`Zigzag`). Nothing currently red goes green.

**Interim, if a constant must stay:** set the GUI and MCP bar to **40** so
the workspace ships one project number instead of two. **Verdict flips:
none** — all three reference projects are over 40 as well as over 20. This
change removes a contradiction without silencing anything.

### 5.7 P8 — narration reads the op's band

`narrate.rs` already has `context.operation_kind`. Replacing the bare
`AIR_CUT_WARNING_PERCENT` comparison with `air_cut_high_threshold_pct()` —
falling back to 50.0 when the op returns `None` — makes the ⚠ agree with the
gate, which is what the D7 ruling's own comment says the marker is for.

**Verdict flips, under P1 (45):** none.

| Row | now (50) | under the band | |
|---|---|---|---|
| golden `[Zigzag]` 90.9 | ⚠ | ⚠ (band 40) | unchanged |
| golden `[Pocket]` 10.6 | ℹ | ℹ | unchanged |
| golden `[DropCutter]` 34.3 | ℹ | ℹ (band 45) | unchanged |
| golden `[Waterline]` 54.3 | ⚠ | ⚠ (band 45) | unchanged |
| wanaka tp5 50.77 | ⚠ | ⚠ (band 40) | unchanged |
| wanaka tp8 35.71 | ℹ | ℹ (band 45) | unchanged |
| wanaka tp9 55.83 | ⚠ | ⚠ (band 45) | unchanged |

**P8 is order-dependent and must not land before P1.** At the *current* 30,
wanaka tp8 (35.71) and the 3D golden's `[DropCutter]` (34.28) would flip
ℹ → ⚠ — P8 alone would *add* warnings.

### 5.8 P9 — the `< 0.02` classification stays

Every percentage in this document is built on it. Moving it moves all eight
bars at once, invalidates both goldens and every number here, and the
evidence for a different value is nil. Untouched.

---

## 6. Bars proposed unchanged, and why — summary

| Bar | why unchanged |
|---|---|
| 2.5D clearing 40 | Clean band tops out at 35.5 (`Adaptive`); 40 is the only value that neither false-alarms on adaptive nor silences wanaka tp5. Present-day catches (tp5, stacked `Zigzag`) are true positives. |
| 2D contour 40 | Nothing measurable in the family exceeds 20.4. The band is free headroom; the real exposure is the `VCarve`/`Inlay` abstention dependency, which wants a sentry, not a number. |
| drill / pin-drill `None` | Out of scope; drill-native gates are the channel. |
| CLI 40 | Already the higher of the two shipped project constants. Recommended to become derived, but if it stays a constant it stays at 40. |
| per-sample `< 0.02` | The substrate of every number here; no evidence for a different value. |

---

## 7. Open risks and what could not be measured

| id | What | Why it matters |
|---|---|---|
| **R1** | **`Rest` has no post-flip reading.** It sits in the 40 band and needs a two-op cascade with `claims_reference = machined_stock` and a sim cell well under the tool tip radius (`feedback_rest_measurement_prerequisites`). Not built. | The 40 band is proposed unchanged partly on a family member never measured. If a rest pass reads 45 % on clean geometry the P3 recommendation is wrong. |
| **R2** | **`Pencil` could not be measured synthetically** — a smooth hemisphere has no concave seams and the op emits nothing. Its only post-flip reading is wanaka tp9 = 55.83. | P1's headline claim, "tp9's flag survives and that is correct", rests on **one** number. A pencil fixture with real valleys would either confirm 45 or argue for a `Pencil`-specific band. |
| **R3** | A `Pencil`-specific band was **considered and not proposed**. Pencil is arguably sparse-by-construction like `ProjectCurve`, which would argue for 60–70 rather than 45. | Cannot calibrate a band from one data point; naming the option so the user can ask for the fixture. |
| **R4** | **The harness's isolation choice inflates the 3D finish family.** No row was preceded by a roughing pass. A real finish stack would read lower. | P1's cluster (34–43) is therefore an **upper** estimate of clean-geometry finish air, which makes 45 conservative in the safe direction — but a stacked fixture could show the cluster at 25–30 and make 45 far too loose. |
| **R5** | **`ProjectCurve` did not stabilise** (§4.b, −15.6 pp across two cell sizes, sign-inverted vs the old kernel). | P2's 60 is deliberately loose for this reason. Anyone tightening it later needs a bigger fixture first. |
| **R6** | **`VCarve` / `Inlay` correctness depends on the abstention rule**, not on the band (§5.4). | An invisible coupling: narrowing `sim_measurability` would turn on permanent warnings for every sign-carving project with no threshold edit in the diff. |
| **R7** | **`Adaptive3d` has only wanaka's two readings** (16.73, 50.77) and **no isolated one** — the harness's flat arm carries `Adaptive`, not `Adaptive3d`. | The 3× intra-project asymmetry is the strongest argument for keeping 40, and it comes from a single project. |
| **R8** | **Not re-measured:** the 0.31 % → 89.61 % cell sweep from `DELTA_sim_w5_s1_DECISION.md` §2. This package measured *different* fixtures and independently found old-kernel spreads of **+23 to +62 pp** across 0.5 → 1.0 mm against swept spreads of **+0.7 to +4.0 pp**. Consistent with the claim; not a reproduction of it. | Stated so nobody cites this document as having verified that specific number. |
| **R9** | While measuring, `cargo test -p rs_cam_core --release` emitted three `dead_code` warnings from `dexel_stock/stamping.rs` (`PlaybackPartial`, its `empty`/`merge`, and `stamp_segment_on_band`) against an in-flight S6 working tree. `cargo clippy --workspace --all-targets -- -D warnings` was **clean** at the same moment, and the S6 lane has since committed. Not this lane's files. | Recorded so the observation is not lost; not acted on, and possibly already resolved. |

---

## 8. What landed with this package

| Commit | What |
|---|---|
| `b5f04331` | `crates/rs_cam_core/tests/air_cut_family_calibration_w5bf4.rs` — 38-row family table (2 kernels × 2 cell sizes) plus the triage-composition probe of §4.d. No threshold moved. Clippy clean at `--workspace --all-targets -D warnings`, `rustfmt --check` clean. |

Nothing in `catalog.rs`, `sim_diagnostics.rs`, `cli/project.rs`,
`viz/compute.rs`, `narrate.rs` or `sim_triage.rs` was touched.

Reproduce the tables:

```text
cargo test -p rs_cam_core --release --test air_cut_family_calibration_w5bf4 -- --nocapture
RS_CAM_STAMP_DISPATCH=whole_path \
  cargo test -p rs_cam_core --release --test air_cut_family_calibration_w5bf4 -- --nocapture
```
