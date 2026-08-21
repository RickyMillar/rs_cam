# W5B-F3 (corpus half) — the acceptance corpus and the rest-chain geometry
# diff under the swept stamp default

Closes the half of `DELTA_sim_w5b_landing.md` §9 W5B-F3 that the param-sweep
lane left open. The sweep half is already closed (56/56 green, 2026-08-21).
This lane answers the other two questions:

* **(a)** what the swept default does to the 18-row acceptance smoke corpus, and
* **(b)** what it does to generated **geometry** on `FromRemainingStock` chains
  beyond wanaka200.

Branch `tech-debt-3`, measured at `8fb8b119` on 2026-08-21. **No production code
changed in this lane.** The only code commit is the A/B harness gaining three
env hooks.

---

## 0. Headline

| | |
|---|---|
| **New nonzero collision counts** | **NONE.** Every arm of every run, project and per-toolpath, `collision_count = 0` and `rapid_collision_count = 0`. There is no STOP-AND-REPORT finding in this lane. |
| Smoke: swept-attributable status changes | **0** |
| Smoke: swept-attributable verdict-kind changes | **2**, both `chipload exceeds_high → within` (AS009 chamfer, AS010 inlay) — a *relaxation*, mechanism identified in §2.c |
| Smoke: swept-attributable metric movements | **33 cells over 11 cases**, every one in the classified (i)/(ii-a)/(ii-b) directions |
| Rest-chain ops A/B'd | **8** across two projects (4 + 4) |
| Rest-chain ops moving the WRONG way | **0** — no new collisions, no removal drop outside the instrument band, no runtime increase on any rest op |
| Things that did not reproduce / are broken | **4**, §5 — one of them makes the corpus runner's chipload regression net **vacuous** |

---

## 1. Method

### The three-way attribution

The shipped acceptance baseline is `2026-06-04.csv` — eleven weeks old, and the
intervening months contain at least four deliberate, documented, metric-moving
changes (the 2026-08-06 chipload unit deletion, the F-024 axial frame fix, the
R-plane peck rooting, the identity-setup frame fix). A two-way diff against it
cannot say what **swept** did.

So every smoke number below is measured **three ways, one binary, one session**:

| arm | what it isolates |
|---|---|
| `2026-06-04.csv` → `RS_CAM_STAMP_DISPATCH=whole_path` | the intervening eleven weeks, **swept excluded** |
| `whole_path` → default (`Auto` = `Swept`) | **swept, and only swept** — paired, same binary, same session |

The same discipline applies to part (b): each project is run twice back to back
in one session, one dispatch mode per process (the mode is a `OnceLock`, so one
process is one mode).

### Reproducibility

The swept smoke arm was run **twice** and the two output CSVs are
**byte-identical**. The deltas below are therefore not run-to-run noise.

### Invocations

```text
# (a) — resolution 0.5 mm (the runner's own default)
                              ./target/release/rs_cam_cli smoke --output swept.csv
RS_CAM_STAMP_DISPATCH=whole_path ./target/release/rs_cam_cli smoke --output wholepath.csv
./target/release/rs_cam_cli smoke --diff --baseline <a> --output <b>

# (b) — one process per arm, harness `tests/swept_wanaka_ab_s1.rs`
S1AB_PROJECT=<toml> S1AB_RESOLUTION_MM=<mm> [RS_CAM_STAMP_DISPATCH=whole_path] \
  cargo test -p rs_cam_core --release --test swept_wanaka_ab_s1 -j 8 -- --ignored --nocapture
```

Every cargo invocation ran under `flock /tmp/rs_cam_cargo.lock` at `-j 8`
behind a ≥20 GB available-memory gate; the four long sims ran strictly
sequentially, never two at once.

### Cell sizes, and why

| project | cell | why |
|---|---:|---|
| `wanaka200_unified_finish.toml` | **0.40 mm** | identical to reference 0D on the sibling `wanaka200.toml`, so the shared setup-1 ops are cross-comparable against a published number. They are, exactly — see §3.a. |
| `wanaka.toml` (wanaka100) | **0.25 mm** | its finishing tool is a tapered ball with a **0.5 mm tip radius**, and the rest-measurement prerequisite is *cell ≪ tip radius*. 0.4 mm would not clear it. The stock is 140×150 against wanaka200's 240×250, so 0.25 mm costs fewer columns than 0D's 0.4 mm did. |

`wanaka.toml` has **uncommitted local modifications** and was used as found, per
the brief:

```text
 planning/airrun_2026-06-01/wanaka.toml | 119 +++++++++++++++++++++++++++++++--
 1 file changed, 114 insertions(+), 5 deletions(-)
```

The material changes are: a new enabled `unified_finish` toolpath (id 15) with
`stock_source = "from_remaining_stock"`, `drop_cutter` id 11 flipped
`enabled = true → false`, three previously-disabled `debug_options` flipped on,
`machine_ref` dropped, and `segment_merge` / `trochoid_cap_mult` /
`engagement_measure` / `junction_deviation_mm` keys added.

**Rest-op count correction.** The brief says wanaka.toml has 6 rest ops. It has
**six `from_remaining_stock` rows, of which four are enabled** (ids 5, 6, 10, 15;
ids 11 and 12 are disabled). Only enabled ops generate, so this lane A/B's four.

---

## 2. Part (a) — the acceptance smoke corpus

18 cases, `planning/toolpath_acceptance/cases_agent_smoke.csv`, runner default
resolution 0.5 mm.

### 2.a Classification of the swept-attributable movement

Paired `whole_path` → `swept`, same binary. **33 numeric cells + 2 verdict
kinds moved, across 11 of the 18 cases.** Every one falls in a class the
landing doc already named.

| class | column | cases | direction | count |
|---|---|---|---|---:|
| **(ii-b)** engagement stops being a function of cell size | `avg_engagement` | AS001–005, 007, 008, 009, 010, 013, 014, 017 | **up in all 11** | 11 |
| **(ii-a)** the commanded axial DOC is reached | `peak_axial_doc_mm` | AS004, 008, 009, 010, 013, 014, 017 | **up in all 7** | 7 |
| **(ii-b)** downstream of engagement | `power_peak_kw` | AS001–004, 009, 010, 013, 014, 017 | up in all 9 | 9 |
| **(ii-b)** downstream of engagement | `deflection_peak_mm` | AS004, 009, 010, 013, 017 | up in all 5 | 5 |
| **verdict kind** | `chipload_kind` | AS009, AS010 | `exceeds_high` → **`within`** | 2 |
| — | `rapid_collision_count` | none | **all 0 in both arms** | 0 |
| — | `status` | none | — | 0 |
| — | `chipload_observed_mm_tooth` | none | **bit-identical in both arms** | 0 |
| — | `deflection_kind`, `power_kind` | none | — | 0 |
| — | all four `drill_*` columns | none | — | 0 |

Three of those rows are worth reading as results rather than as table entries:

* **`chipload_observed_mm_tooth` does not move at all.** That is the
  2026-08-06 unit deletion working as documented: the observation is now
  `effective_feed ÷ (rpm · flutes)`, which contains no dexel input, so it is
  dispatch-invariant by construction. The corpus confirms it on 12 cases.
* **The drill columns do not move at all.** Drill ops bypass dexel stamping for
  analytical cone/cylinder removal, so no stamp kernel can reach them. Also
  confirmed by construction *and* by measurement.
* **`peak_axial_doc_mm` moves up on 7 cases and down on none.** That is exactly
  the (ii-a) claim — the old kernel under-reads the commanded depth. On AS004
  (face, `depth_per_pass = 0.5`) swept reads **exactly 0.500** against
  whole_path's 0.438. On the five cases whose commanded `depth_per_pass`
  already read exactly (AS001/2/3/5 at 2, 3, 2, 2 mm) neither arm moves.

### 2.b The one case where swept overshoots the commanded depth

`AS013` (adaptive3d, `depth_per_pass = 3`): `3.000 → 3.743`, i.e. **+24.8% over
the commanded pass depth**. This is the residue the F-XXX sentries' split bar
was widened for in the landing wave (`dpp + 1.0` = 4.0 absolute ceiling), and
3.743 sits inside it with 6.4% margin. Recorded because a reader who only has
the landing doc's "the old kernel under-reads" framing would not expect the new
one to read *over* the commanded value.

### 2.c The two verdict flips, and why the observation did not move

AS009 (chamfer) and AS010 (inlay) both go `chipload exceeds_high → within`
while `chipload_observed_mm_tooth` is **identical to six decimal places** in
both arms (0.030000 and 0.026667). A verdict that moves with its observation
pinned means the **band** moved, and it did:

Both cases are V-bit ops. `tool_load::chipload` picks the row to match against
with `tool.lookup_diameter_at(lookup_axial_doc_mm)` (`chipload.rs:550,579`) —
on a V-bit the lookup diameter is a **function of the measured axial DOC**.
Swept measures a deeper cut (AS009 `0.504 → 1.100`, AS010 `0.502 → 1.763`), so
the effective diameter grows, the transferred band widens, and the same
advance-per-tooth stops exceeding it.

**Direction of the change: the gate gets quieter.** It is not a false alarm
being removed — it is a bar moving out from under an unchanged observation
because a *different* measurement got better. Two things follow:

1. It is not a regression in the `smoke --diff` sense (within → exceeds is the
   regression direction; this is the reverse), and the tool agrees.
2. It is a live example of the sub-Ø2 provisionality `CLAUDE.md` already flags:
   the `D^0.61` diameter law is repo-derived, no primary source publishes it,
   and here it is the thing deciding the verdict. A bench measurement moving
   that exponent moves both of these verdicts back.

**Not an air-cut-driven flip.** No smoke verdict in either arm is produced by
an air-cut band, so nothing here feeds the W5B-F4 threshold decision
(`DELTA_w5b_f4_aircut_DECISION.md`). The corpus is single-op cases with no
prior roughing; the air-cut bands are a `ProjectDiagnostics` surface the smoke
runner does not emit.

### 2.d What the intervening eleven weeks did (swept EXCLUDED)

Measured `2026-06-04.csv` → `whole_path`, i.e. everything **except** swept.
It is much larger than the swept delta and is listed so nobody attributes it
here.

| change | cases | note |
|---|---|---|
| `chipload_observed_mm_tooth` up **4.0×–14.7×** | all 12 measured cases | the 2026-08-06 unit deletion. Inside `CLAUDE.md`'s stated 2.4×–40.4× / median 10.9× band. |
| `chipload_kind` moved | 8 | 3 × `exceeds_low → within`, 5 × `exceeds_low → exceeds_high` |
| `deflection_peak_mm` fell by up to **28×** | 9 | AS017 1.2855 → 0.0453 mm |
| `deflection_kind exceeds → within` | AS014, AS017 | two gate relaxations |
| **`rapid_collision_count` → 0 on every case that had one** | AS007 6→0, AS009 1→0, AS010 **104→0**, AS017 **100→0** | see the warning below |
| `peak_axial_doc_mm` fell | AS013 **14.766 → 3.000**, AS017 16.504 → 10.145 | the F-024 identity-setup frame fix landing on the corpus; 3.000 is exactly the commanded `depth_per_pass` |
| `drill_chip_welding_observed` 4.000 → 1.000 | AS011 | the R-plane peck rooting |
| `status ok → generation_failed` | AS015 | §5.a |

> **Flagged, not investigated: 211 rapid collisions became 0 without swept.**
> Every one of those four counts going to zero is *probably* the identity-frame
> and export-datum fixes doing exactly what they were built to do — the two
> largest (AS010, AS017) are precisely the frame-sensitive cases. But
> `CLAUDE.md`'s own rule is that a gate handed an empty population passes and
> looks healthy, and "the collision count went to zero" is the shape that rule
> warns about. This lane did **not** confirm the collision detector still has a
> population on those cases. It is outside W5B-F3's scope and belongs to
> whoever re-baselines the corpus. It is **not** attributable to swept: both
> arms read 0.

### 2.e Verdict on part (a)

**Swept introduces no regression into the acceptance corpus.** The official
diff agrees — `smoke --diff --baseline wholepath.csv --output swept.csv` reports
`no regressions (18 cases checked)` and exits 0 — though see §5.b for why that
particular green is weaker than it looks and the manual column-by-column diff
above is the load-bearing evidence.

The corpus baseline itself is stale and should be re-cut; that is a separate
item, and it must be cut **after** someone answers the collision question in
§2.d, not before.

---

## 3. Part (b) — rest-chain geometry

Two projects, four `FromRemainingStock` ops each, paired same-binary A/B.

### The built-in control, and how to read a removal delta

Each project contains ops with `stock_source = "fresh"` whose emitted geometry
**cannot** change under a stamp-kernel swap. In both projects their
`move_count`, `cutting_distance_mm` and `rapid_distance_mm` came back
**bit-identical across the two arms**, which is the non-vacuity check for the
whole exercise: it proves the harness really did A/B one variable.

Those same ops still show a *measured* removal change, because swept measures
removal differently (class (i)). That gives a calibrated **instrument band**:
any rest op whose removal moves by less than the Fresh ops' movement has not
demonstrably removed more or less material — it has been measured differently.

| project | Fresh-op removal movement (pure instrument) |
|---|---|
| `wanaka200_unified_finish` | tp1 **−0.73%**, tp5 **−2.36%** |
| `wanaka.toml` | see §3.b |

**Read every removal delta below against that band, not against zero.**

### 3.a `wanaka200_unified_finish.toml` — 0.40 mm, 8 toolpaths, 4 rest ops

Ladder converged in 2 rounds, 0 pending, `effective_cell_mm` 0.400,
`resolution_clamped` 0, in **both** arms.

**Cross-check against published 0D.** This project shares setup 1 and the front
rough with `wanaka200.toml`. Every one of those ops reproduces reference 0D
**to the printed digit** in the swept arm — tp1 removal 586 083.9965 vs 0D's
586 084.0, air 16.7347 vs 16.73, avg eng 0.1592, peak axial 16.2528 vs 16.253,
moves 11 761, rapid 19 944.6292 vs 19 944.6. 0D reproduces.

#### Safety

| | whole_path | swept |
|---|---:|---:|
| `collision_count`, project and all 8 toolpaths | **0** | **0** |
| `rapid_collision_count`, project and all 8 toolpaths | **0** | **0** |

#### The four rest ops

| tp | op | metric | whole_path | swept | Δ |
|---|---|---|---:|---:|---:|
| **tp3** | `project_curve` (rivers, V-bit) | move_count | 4 365 | 4 365 | **0** |
| | | cutting mm | 4 413.50 | 4 430.33 | +0.38% |
| | | rapid mm | 13 061.95 | 13 045.11 | −0.13% |
| | | removal mm³ | 8 324.01 | 8 659.63 | **+4.03%** |
| | | air % (total) | 16.05 | 15.97 | −0.08 pp |
| | | peak axial mm | 2.295 | 3.429 | +49.4% |
| | | runtime s | 702.51 | 704.30 | +0.26% |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |
| **tp4** | `project_curve` (lakes, R1.0) | move_count | 1 184 | 1 184 | **0** |
| | | cutting mm | 1 034.3425 | 1 034.3531 | +0.001% |
| | | rapid mm | 1 535.8904 | 1 535.8798 | −0.001% |
| | | removal mm³ | 7 332.36 | 7 558.23 | **+3.08%** |
| | | air % (total) | 10.55 | 10.90 | +0.35 pp |
| | | peak axial mm | 3.999 | 5.143 | +28.6% |
| | | runtime s | 164.38 | 167.34 | +1.80% |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |
| **tp6** | `unified_finish` (R1.5) | move_count | 233 117 | 233 657 | +0.23% |
| | | cutting mm | 93 427.29 | 94 127.50 | +0.75% |
| | | **rapid mm** | **62 268.16** | **46 373.18** | **−25.5%** |
| | | removal mm³ | 40 623.56 | 44 205.77 | **+8.82%** |
| | | air % (total) | 52.69 | 35.48 | **−17.2 pp** |
| | | peak axial mm | 2.562 | 3.104 | +21.1% |
| | | runtime s | 8 023.36 | 7 630.94 | **−4.89%** |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |
| **tp7** | `pencil` (R0.5) | move_count | 39 872 | 40 483 | +1.53% |
| | | cutting mm | 45 470.39 | 45 427.29 | −0.10% |
| | | rapid mm | 137 738.48 | 140 999.11 | **+2.37%** |
| | | removal mm³ | 10 278.09 | 10 222.21 | −0.54% |
| | | air % (total) | 59.26 | 56.05 | −3.21 pp |
| | | peak axial mm | 2.264 | 3.097 | +36.8% |
| | | runtime s | 29 937.23 | 29 796.65 | −0.47% |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |

**tp3 and tp4 did not change path — they changed the feed/rapid split point.**
`cutting + rapid` is conserved to the last printed digit in both:
17 475.4448 mm for tp3 and 2 570.2329 mm for tp4, in *both* arms, with
`move_count` unchanged. The generator derived a slightly different
rapid-to-feed transition height off the new remaining-stock grid; the polyline
is the same. 16.8 mm of tp3's travel moved from rapid to feed.

**Verdicts:**

| tp | verdict | reasoning |
|---|---|---|
| tp3 | **benign** | path length and move count conserved; removal +4.03% is above the −0.7…−2.4% instrument band so it is real material, at the cost of 16.8 mm of rapid becoming feed |
| tp4 | **benign** | movement is at the 1e-3 % level on every geometry column |
| tp6 | **better, unambiguously** | −15 895 mm of rapid, −17.2 pp air, **+8.82%** removal (4× outside the instrument band), −4.89% runtime, 0 collisions. This is `wanaka200` tp8's halving reproducing on a different operation family. |
| tp7 | **benign** | the only rest op whose rapid distance goes **up** (+2.37%), but removal −0.54% is *inside* the instrument band (Fresh ops moved −0.73% and −2.36% on the same run), and both air-cut and total runtime went **down**. No wrong-way movement survives the control. |

### 3.b `wanaka.toml` (wanaka100) — 0.25 mm, 7 enabled toolpaths, 4 rest ops

Ladder converged in 2 rounds, 0 pending, `effective_cell_mm` 0.250,
`resolution_clamped` 0, in **both** arms.

Enabled: tp14 `alignment_pin_drill` (Fresh), tp4 `adaptive3d` (Fresh),
tp7 `drill` (Fresh), and four `FromRemainingStock` ops — tp5 and tp6
`project_curve`, tp10 `adaptive3d`, tp15 `unified_finish`.

#### Safety

| | whole_path | swept |
|---|---:|---:|
| `collision_count`, project and all 7 toolpaths | **0** | **0** |
| `rapid_collision_count`, project and all 7 toolpaths | **0** | **0** |

#### Instrument band from the Fresh ops

tp4 (`adaptive3d`, Fresh) has **bit-identical** `move_count` (3 355),
`cutting_distance_mm` (12 602.2223) and `rapid_distance_mm` (2 623.9335) across
the arms, and its measured removal moves **−0.373%**. tp14 and tp7 are drills
and are identical on every column. So the instrument band here is about
**−0.4%**.

#### The four rest ops

| tp | op | metric | whole_path | swept | Δ |
|---|---|---|---:|---:|---:|
| **tp5** | `project_curve` (rivers) | move_count | 2 064 | 2 064 | **0** |
| | | cutting mm | 2 603.4065 | 2 604.3428 | +0.036% |
| | | rapid mm | 7 481.5278 | 7 480.5915 | −0.013% |
| | | removal mm³ | 6 818.56 | 6 843.77 | +0.37% |
| | | air % (total) | 32.39 | 32.06 | −0.33 pp |
| | | peak axial mm | 5.753 | 6.867 | +19.4% |
| | | runtime s | 789.74 | 790.10 | +0.05% |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |
| **tp6** | `project_curve` (lakes) | move_count | 1 928 | 1 928 | **0** |
| | | cutting mm | 1 648.5653 | 1 650.6471 | +0.126% |
| | | rapid mm | 2 557.1580 | 2 555.0762 | −0.081% |
| | | removal mm³ | 5 510.41 | 5 567.80 | +1.04% |
| | | air % (total) | 21.20 | 21.21 | +0.01 pp |
| | | peak axial mm | 5.575 | 5.575 | **0** |
| | | runtime s | 264.61 | 265.26 | +0.25% |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |
| **tp10** | `adaptive3d` (3D rough) | move_count | 2 163 | 2 163 | **0** |
| | | cutting mm | 4 939.1949 | 4 939.1949 | **0 — bit-identical** |
| | | rapid mm | 1 456.9587 | 1 456.9587 | **0 — bit-identical** |
| | | removal mm³ | 37 590.10 | 37 292.55 | −0.79% |
| | | air % (total) | 14.70 | 26.39 | **+11.69 pp** |
| | | peak axial mm | 3.456 | 5.038 | +45.7% |
| | | runtime s | 317.00 | 317.51 | +0.16% |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |
| **tp15** | `unified_finish` (R0.5-tip taper) | move_count | 205 910 | 205 885 | −0.01% |
| | | cutting mm | 60 062.18 | 60 082.58 | +0.034% |
| | | rapid mm | 17 165.77 | 16 750.14 | **−2.42%** |
| | | removal mm³ | 10 119.56 | 10 365.84 | **+2.43%** |
| | | air % (total) | 11.49 | 10.92 | −0.57 pp |
| | | peak axial mm | 2.006 | 2.991 | +49.1% |
| | | runtime s | 8 719.85 | 8 805.59 | +0.98% |
| | | collisions / rapid collisions | 0 / 0 | 0 / 0 | — |

**tp5 and tp6 reproduce project 1's tp3/tp4 pattern exactly**: `cutting + rapid`
conserved to the last printed digit (10 084.9343 mm and 4 205.7233 mm, both
arms), `move_count` unchanged, a small amount of travel reclassified from rapid
to feed. Same polyline, different feed/rapid transition height.

**tp10 is the interesting one: a `FromRemainingStock` op whose generated
geometry is bit-identical across the kernels.** So "swept changes rest-op
geometry" is not universal — an adaptive3d rest pass on this stock produced the
identical path from both grids. Its whole delta (removal −0.79%, air +11.7 pp,
peak axial +45.7%) is instrument, and its removal delta sits at the same scale
as the Fresh control's −0.37%.

**Verdicts:**

| tp | verdict | reasoning |
|---|---|---|
| tp5 | **benign** | path conserved, removal +0.37% against a −0.4% instrument band, air down |
| tp6 | **benign** | same pattern; peak axial did not move at all |
| tp10 | **benign — no geometry change at all** | every emitted-geometry column bit-identical; removal −0.79% is instrument, not material |
| tp15 | **benign, one mild caveat** | −415.6 mm of rapid, **+2.43%** removal (6× the instrument band), 25 fewer moves, air down, 0 collisions. The caveat is `total_runtime_s` **+0.98%** — see §6/§6.a; it is a modulated cycle-time estimate, not path length, and the path got *shorter*. |

#### The direction reversal — swept makes two of this project's air readings WORSE

`wanaka200_unified_finish`'s air-cut readings all fell under swept.
**`wanaka.toml`'s rise**: project `air_cut_pct_of_total_runtime` **13.20 →
14.26**, and project `average_engagement` **falls** 0.2861 → 0.2467. Two ops
drive it:

| tp | op | air % whole_path | air % swept |
|---|---|---:|---:|
| tp4 | `adaptive3d` (Fresh, **geometry bit-identical**) | 9.10 | **28.58** |
| tp10 | `adaptive3d` (rest, **geometry bit-identical**) | 14.70 | **26.39** |

This is the landing doc's own (ii-b) statement working in the other direction —
*"air stops being a function of the cell size, and on a fixture where the old
reading happened to be low the correction goes the other way"* — and it is
worth having on record because both wanaka200 references (0C→0D) and this
project's sibling only ever showed the falling direction.

The project verdict nevertheless goes **`NOT MEASURED` → `OK`**: whole_path
abstained on tp15 (70% of removing samples reading zero engagement); swept can
measure it, and nothing exceeds a band. `not_measurable` rows 3 → 0,
`degraded` rows 3 → 9.

### 3.c Input to W5B-F4

`DELTA_w5b_f4_aircut_DECISION.md` §5 proposes P1 (3D finish 30 → 45), P2
(`ProjectCurve` 97 → 60) and P3 (2.5D clearing + 3D rough **unchanged at 40**).
Eight fresh swept readings from this lane, none of which the package had:

| family | proposed bar | swept readings from this lane | verdict |
|---|---:|---|---|
| `ProjectCurve` | 60.0 | 15.97, 10.90, 32.06, 21.21 | **P2 survives** on all four, worst case 1.9× under |
| 3D finish (`unified_finish`) | 45.0 | 35.48, 10.92 | **P1 survives**; note 35.48 **exceeds today's 30.0**, so P1 is the difference between a flag and no flag on `wanaka200_unified_finish` |
| 3D finish (`pencil`) | 45.0 | 56.05 | **exceeds the proposed bar too** — consistent with the package's R3 (a pencil-specific band was considered and not proposed) |
| 2.5D clearing + 3D rough | 40.0 (unchanged) | 16.73, 50.77, 28.58, 26.39 | P3's headroom is **thinner than the package's data showed**: two adaptive3d ops that read 9.10 and 14.70 under the old kernel read **28.58 and 26.39** under swept. Headroom against 40 drops from ~4.4× to ~1.4×. |

**Do not read the last row as an argument to raise P3.** It is an argument that
P3's margin is now small enough that the next fixture could flip it, and that
whoever takes the decision should know the swept correction moves this family
*up* on some projects.

Also, an explicit non-claim: the package's open question **R1** is about
`OperationType::Rest` having no post-flip reading. This lane measured ops with
`StockSource::FromRemainingStock`, which is a *different axis* — none of the
eight is an `OperationType::Rest`. R1 remains open and these numbers do not
close it.

---

## 4. Wall clock — recorded, NOT load-bearing

`BASELINES.md`'s measurement-discipline rule applies: these are single runs per
arm on a box whose load average moved during the session, and a 22% swing with
no code change is on record. They are here for shape only.

| project | arm | wall |
|---|---|---:|
| `wanaka200_unified_finish` @ 0.40 mm | whole_path | 236.3 s |
| `wanaka200_unified_finish` @ 0.40 mm | swept | 229.2 s |
| `wanaka.toml` @ 0.25 mm | whole_path, modulation on | 201 s |
| `wanaka.toml` @ 0.25 mm | swept, modulation on | 199 s |
| `wanaka.toml` @ 0.25 mm | whole_path, modulation off | 211 s |
| `wanaka.toml` @ 0.25 mm | swept, modulation off | 214 s |

The smoke corpus runs 20 s per arm at 0.5 mm.

---

## 7. Summary of every verdict

### Part (a) — 18 smoke cases

| class | count |
|---|---:|
| swept-attributable **status** regressions | **0** |
| swept-attributable **collision** increases | **0** (all cases 0 in both arms) |
| swept-attributable **verdict-kind** changes | **2**, both relaxations (`chipload exceeds_high → within`, AS009/AS010, §2.c) |
| swept-attributable metric movements, all in classified directions | 33 cells / 11 cases |
| pre-existing regressions vs the 2026-06-04 baseline, present in BOTH arms | **1** (AS015, §5.a) |

### Part (b) — 8 rest ops across 2 projects

| project | tp | op | geometry moved? | verdict |
|---|---|---|---|---|
| `wanaka200_unified_finish` | tp3 | `project_curve` | feed/rapid split only, length conserved | **benign** |
| | tp4 | `project_curve` | 1e-3 % scale | **benign** |
| | tp6 | `unified_finish` | **yes — rapid −25.5%** | **better** |
| | tp7 | `pencil` | yes — rapid +2.4%, moves +1.5% | **benign** |
| `wanaka.toml` | tp5 | `project_curve` | feed/rapid split only, length conserved | **benign** |
| | tp6 | `project_curve` | feed/rapid split only, length conserved | **benign** |
| | tp10 | `adaptive3d` | **no — bit-identical** | **benign** |
| | tp15 | `unified_finish` | **yes — rapid −2.4%, 25 fewer moves** | **better** |

**Nothing moved the wrong way.** The test applied to each op was: new
collisions (none, anywhere), removal drop >2% without an air or rapid win
(none — the two negative removal deltas, tp7 at −0.54% and tp10 at −0.79%, are
both inside the Fresh-op instrument band measured on the same run), or runtime
up (the only two, tp15 +0.98% and project +0.81%, are modulator artefacts and
vanish under the §6.a control while the path itself got shorter).

## 7.a Verification

| gate | result |
|---|---|
| `cargo fmt --check -p rs_cam_core` | clean |
| `cargo clippy -p rs_cam_core --all-targets -- -D warnings` | **zero warnings** |
| harness with **no** env set | names `planning/airrun_2026-08-19/wanaka200.toml`, `0.400000`, `mode=auto`, `adaptive_feed_modulation=1` — reference 0D's configuration, unchanged |
| 0D reproduces | every setup-1 op of `wanaka200_unified_finish` matches 0D's published `wanaka200` rows to the printed digit (§3.a) |
| smoke run repeatability | swept arm run twice, output CSVs **byte-identical** |

Harness commit: `af8c1c48`, test-only, three env hooks all defaulting to the
0D configuration. No production code changed in this lane.

## 8. Follow-ups this lane produced

| id | what | owner |
|---|---|---|
| **F3-1** | The corpus runner's `prior_passes` chain needs a `run_simulation` between the prior pass and the measured case, or AS015 stays dead. Broken since `4b105dab` (2026-07-06). | corpus |
| **F3-2** | `smoke::run_diff`'s `is_exceeds` never matches the chipload or drill-gate spellings — that arm of the F-037 regression net is vacuous. | corpus |
| **F3-3** | Re-cut the acceptance baseline. **After** F3-1, F3-2, and after someone confirms the 211→0 rapid collisions in §2.d are a fix and not a blind detector. | corpus |
| **F3-4** | `session/compute.rs:2352-2355` states `adaptive_feed_modulation`'s default is `false`; `session/mod.rs:878` sets it `true`. One-line doc fix. | sim |
| **F3-5** | Record in the landing doc / `CLAUDE.md` that the swept default changes emitted **feeds** on every cutting toolpath, not only geometry on rest chains (§6.a). | consolidator |
| **F3-6** | W5B-F4 input: P3's headroom against 40.0 is now ~1.4×, not ~4.4×, on two adaptive3d ops (§3.c). | W5B-F4 |

---

## 5. What did not reproduce, and what is broken

### 5.a The corpus runner's prior-pass chaining has been dead since 2026-07-06

`AS015` (scallop) is `ok` in the `2026-06-04` baseline and
**`generation_failed` today in BOTH arms**:

```text
Operation failed: 'scallop smoke' is set to use remaining stock (rest machining)
but no simulated remaining-stock snapshot is available. Run a simulation of the
preceding operations first, then regenerate …
```

`smoke.rs`'s documented methodology (the `prior_passes` doc comment, round-10
STATE.md) is: add the prior pass with `StockSource::Fresh`, generate it, then
materialize the measured case with `StockSource::FromRemainingStock` — "All run
in a single `run_simulation` call so the dexel state chains."

The code does exactly that (`smoke.rs:463-496`) and it **no longer works**,
because generating a `FromRemainingStock` op now *refuses* without a prior
simulated snapshot. That refusal landed in **`4b105dab` (2026-07-06)**, six
weeks before swept. So:

* AS015 is the corpus's only rest-chain row and it has produced no measurement
  for ~7 weeks.
* The refusal is correct behaviour; the runner is what is stale — it needs a
  `run_simulation` between the prior pass and the measured case.
* **Not attributable to swept.** Identical under `whole_path`.

### 5.b The smoke diff's chipload regression check is vacuous

`run_diff` decides a verdict regression with:

```rust
fn is_exceeds(kind: &str) -> bool { kind == "exceeds" }
```

but the chipload column never emits bare `"exceeds"` — it emits
`format!("exceeds_{}", side_str(side))`, i.e. `exceeds_low` / `exceeds_high`
(`smoke.rs:595-598`). The drill gate columns are the same shape
(`exceeds_{severity}`) and are not compared by `run_diff` at all.

**Consequence:** a `within → exceeds_high` chipload flip is silently **not** a
regression. Deflection and power do emit bare `"exceeds"`, so those two arms of
the net work. This is the vacuous-gate class `CLAUDE.md` names, on the corpus's
own regression net. It did not hide anything in this lane — the two chipload
flips went the safe direction and the manual diff caught them — but the
`no regressions (18 cases checked)` line cannot be cited as chipload evidence.

### 5.c `SimulationOptions::adaptive_feed_modulation` — the doc comment states
the opposite of the default

`session/compute.rs:2352-2355` says the F-036b post-pass is

> Inert when: `opts.adaptive_feed_modulation == false` (**the default**; the
> smoke baseline and every legacy test pass with this branch skipped,
> byte-identical).

`session/mod.rs:878` sets `adaptive_feed_modulation: true` in
`impl Default for SimulationOptions`. The comment is stale and it matters,
because that post-pass **swaps the modulated toolpath into `self.results` so the
G-code emitter picks up per-move modulated feeds** (`compute.rs:2344-2350`) and
re-times the trace (`compute.rs:2844-2887`). A reader who trusts the comment
will conclude the default simulation path cannot change emitted feeds. It can.

See §6 for the measurement that surfaced it.

### 5.d The brief's rest-op count for `wanaka.toml`

Six `from_remaining_stock` rows, **four enabled**. Stated in §1.

---

## 6. The finding this lane did not go looking for: swept moves emitted FEEDS,
not just rest-op geometry

`wanaka200_unified_finish` tp1 (`adaptive3d`, `stock_source = "fresh"`) has
**bit-identical** `move_count` (11 761), `cutting_distance_mm` (127 363.2460),
`rapid_distance_mm` (19 944.6292) and `sample_count` (680 764) in both arms —
and its `cutting_runtime_s` moves **6 103.94 → 8 052.75 s, +31.9%**.

A time cannot move while the geometry, the sample count and the commanded feeds
are all fixed: `segment_time_s = segment_len / feed_rate_mm_min × 60`
(`dexel_stock/simulation.rs:738`). So a **feed** moved.

The only shipped path that moves a feed after generation is the F-036b
adaptive feed-modulation post-pass, which `run_simulation` executes **by
default** (§5.c), reading the measured per-move engagement and writing the
re-solved feeds back into the cached toolpath. Swept raises tp1's measured
`average_engagement` by **+40.5%** (0.1134 → 0.1592), and on a rest chain the
F.4 ladder runs `run_simulation` three times, so the pass is applied three
times, each round reading the previous round's engagement.

The direction is not uniform, which is why it needed a control rather than a
story: tp5 (`adaptive3d`, also Fresh, also bit-identical geometry) moves its
engagement **+74.9%** and its `cutting_runtime_s` **−1.7%**. `ConstrainedMax`
can raise a feed as well as lower one.

### 6.a The control — confirmed

The harness gained an `S1AB_FEED_MODULATION` hook and `wanaka.toml` was run a
**second** paired A/B with the post-pass disabled. Four arms, one session,
sequential, same binary:

| arm | dispatch | F-036b post-pass |
|---|---|---|
| A | `whole_path` | **on** (shipped default) |
| B | default = `swept` | **on** |
| C | `whole_path` | **off** |
| D | default = `swept` | **off** |

**Step 1 — the knob is not inert.** A vs C (dispatch held fixed, modulation
toggled) on tp4, whose `move_count`, `cutting_distance_mm`,
`rapid_distance_mm` and `total_removed_volume_est_mm3` are identical between
them:

| tp4 | modulation on | modulation off | Δ |
|---|---:|---:|---:|
| `cutting_runtime_s` | 577.328 | **221.404** | **−61.7%** |
| `total_runtime_s` | 674.903 | **459.030** | **−32.0%** |
| `total_removed_volume_est_mm3` | 149 330.907 | 149 330.907 | **0** |

**Step 2 — the decisive one.** C vs D is the *same* stamp-kernel A/B as A vs B,
with the modulator out of the loop. On every op whose emitted geometry is
bit-identical across the kernels, **every runtime becomes bit-identical too**:

| op | geometry across kernels | `cutting_runtime_s` C → D | `total_runtime_s` C → D | same columns, modulation ON |
|---|---|---|---|---|
| tp4 `adaptive3d` (Fresh) | bit-identical | 221.4043 → **221.4043** | 459.0298 → **459.0298** | +0.033% / −0.037% |
| tp10 `adaptive3d` (rest) | bit-identical | 80.8131 → **80.8131** | 246.9993 → **246.9993** | +0.812% / +0.160% |
| project total | — | — | 9 407.52 → 9 397.15 (**−0.11%**, and tp5/tp6/tp15 really did change path) | 10 766.11 → 10 853.12 (**+0.81%**) |

**The mechanism is confirmed.** Swept changes the measured engagement; the
F-036b post-pass reads that engagement and re-solves the per-move feeds; the
re-solved toolpath is swapped into `self.results`, which is what the G-code
emitter and the *next ladder round* read. With the post-pass off, the stamp
kernel moves no time on any op whose path it did not move.

Two consequences worth stating separately:

1. **`wanaka.toml`'s entire +0.81% project cycle-time increase under swept is a
   modulation effect, not a longer path.** With the modulator off the same A/B
   reads **−0.11%**. The `total_runtime_s` +0.98% on tp15 flagged as the one
   mild caveat in §3.b is the same thing — its emitted path got *shorter*
   (−415.6 mm of rapid, 25 fewer moves).
2. **The blast radius of the swept default is wider than the landing doc
   states.** `DELTA_sim_w5b_landing.md` §6 records the accepted risk as "swept
   changes the simulated grid, so `FromRemainingStock` generators emit
   different motion". True, and confirmed here. But swept also changes emitted
   **F-words on every cutting toolpath, `FromRemainingStock` or not**,
   including ops whose XYZ is bit-identical — because the default
   `SimulationOptions` runs a feed solver over the dexel's engagement reading.
   That is emitted G-code on ops nobody expected to move.

Neither is a defect and nothing here moves the wrong way — modulation is
supposed to respond to engagement, and a *better* engagement measurement
producing different feeds is the system working. It is recorded because a
reader who believes "Fresh ops are unaffected by the stamp kernel" would be
wrong about the G-code, and because §5.c's stale doc comment actively teaches
that belief.

---
