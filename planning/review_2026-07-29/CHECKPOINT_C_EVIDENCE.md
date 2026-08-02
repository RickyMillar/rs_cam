# Checkpoint C evidence — M4, "repair the scallop algorithm instead of stacking compensations"

**Research only. No production behaviour changed in this wave.**
`ScallopStepoverPolicy::SHIPPED` is the only value any production entry point
passes, and `shipped_policy_reproduces_the_shipped_path_byte_for_byte` asserts
that it reproduces the shipped toolpath fingerprint on every fixture. PR-3's
scallop fingerprints, Checkpoint B's own harness, and all 56 param sweeps are
unchanged.

HEAD at time of run: `28503db` (branch `experiment/adaptive-spiral`).

| | |
|---|---|
| Instrument | `tests/common/scallop_oracle.rs` |
| Oracle validation | `tests/scallop_oracle_validation_m4.rs` (9 tests, all non-ignored) |
| Candidates | `tests/scallop_candidates_m4.rs` (3 guards + 4 `#[ignore]` evidence runs) |
| Research seam | `scallop::ScallopStepoverPolicy`, `scallop_isofield` |
| Maps | `target/m4_scallop_oracle/` (98 PNGs) |

Reproduce:

```bash
cargo test --release -p rs_cam_core --test scallop_oracle_validation_m4 -- --nocapture
cargo test --release -p rs_cam_core --test scallop_candidates_m4 -- --ignored --nocapture --test-threads=1
```

Tool throughout: the wanaka taper (Ø1 tip / 7° / Ø6 shank, `cusp_radius` 0.5 mm),
commanded cusp **0.020 mm**, tolerance 0.10 mm — deliberately Checkpoint B's
configuration so the two evidence packs compare directly.

---

## 1. Phase A — the oracle

### 1.1 Why a new one

M4's phase A says: improve the oracle first. Every scallop quality judgement
on record was made through an instrument with a known artefact.

| prior instrument | its artefact |
|---|---|
| Checkpoint B `cusp p50/p95` | infers cusp from the **spacing between points on adjacent rings** and pushes it through the *flat* formula. A statement about the tool-centre field, never about stock — and on a slope the same spacing leaves a different cusp, which is the variable M4 is investigating. |
| Checkpoint B `residual_*` | compares emitted move Z against a sampled 0.05 mm reference grid. §5.1 of that document states the trap: scallop's Z is already an exact per-point drop-cutter query, so the column measures the *reference grid's* interpolation error at the points the path visits — **denser paths score worse for free**. |
| `ScallopReport::uncut_core_mm2` | the cascade's own self-report of polygon area it never reached. Cannot see material a ring passed over and did not remove. |
| dexel COLUMNS | trustworthy but grid-quantised, stock-history dependent, and one full simulation per arm. |

### 1.2 What it computes

The machined surface is the lower envelope of the swept cutter. For a cutter
whose profile is `height_at_radius(ρ)` — C3's unified profile accessor, so a
tapered ball is scored with its real cone flank rather than a ball twin's —
one cutter position with its tip at `(cx, cy, cz)` leaves

```
z_machined(x, y) = cz + height_at_radius(ρ),   ρ = hypot(x−cx, y−cy)
```

and the surface after the whole path is the **minimum** over every position.
That is exact analytic geometry: no dexel grid, no Z quantisation, no probe
ball, no sampled reference field. Its only two discretisations are the report
grid (`cell`) and the spacing at which cutting chords are resampled into
positions (`path_step`) — and §1.3 measures both.

```
residual = z_machined − z_true − stock_to_leave
```

The oracle separates four things the old instruments conflated:

* **on-dial cusp** — `|residual| ≤ dial`;
* **over-dial cusp** — `residual > dial`;
* **standing material** — `residual > 5 × dial`, i.e. a pass covered it and
  did not take it down;
* **untouched** — no cutter position ever covered the cell. This is the shape
  `max_rings` truncation actually produces, and conflating it with a cusp
  defect is how a truncated cascade can look like a quality problem.

Residuals are reported both **vertically** (what a dexel simulator sees) and
**surface-normal** (`vertical · cos θ`, what the `scallop_height` dial
promises). Every headline number below is the surface-normal one.

`achieved cusp` is the **p99** of the residual. On a field of passes at
uniform spacing the residual has a closed-form CDF, so p99 = 0.98 × the peak —
pinned by test, not assumed. The p99 is used rather than the max because it is
robust to a single sharp-feature cell.

### 1.3 Validation — nine closed-form checks

| # | claim | result |
|---|---|---|
| 1 | max residual **is** the cusp `h = R − √(R² − (d/2)²)` | **0.00% error** |
| 2 | quantiles follow `r_q = R − √(R² − (q·d/2)²)`; p99/max = 0.98 | ≤ 2.1% at p50/p90/p99 |
| 3a | converges in grid cell (0.020 → 0.010 → 0.005 mm) | 0.00% at every cell |
| 3b | converges in path step (0.080 → 0.020 → 0.005 mm) | 8.35% → 0.52% → 0.00%, coarse over-reports as predicted |
| 4 | a deliberate uniform 100 µm gouge | reads **−100.0 µm** |
| 5 | untouched vs standing are separated | half-covered field: 0.78 mm² untouched, 0.20 mm² standing (the ball's rim fillet); stepped field: 0.00 untouched, 0.70 standing |
| 6 | M3 tile-raster true-surface reference error | p50 **0.26 µm**, p99 **0.85 µm** — no candidate difference below this is real |
| 6b | tool-reach floor error is mesh faceting | **−28.6 → −6.4 → −0.0 µm** as the fixture mesh refines 0.20 → 0.10 → 0.05 mm |
| 7 | **the slope law** (§1.4) | ≤4.5% at 0–60° |
| 8 | tapered stamp radius cap safe below flank contact | identical to 3 decimal places at 0/30/60° |

M3's tile-raster classifier (`0711568`, 187×) is what makes the oracle
affordable at a grid fine enough to resolve a 20 µm cusp. That landing is load
bearing here.

**A harness bug caught and written into the test.** The first draft of the
slope-law test read 1.76× the law at 45°. The pass that makes tangent contact
at a point sits `R·sin θ` **downhill** of it, so a window that runs to the last
pass reports an edge artefact. The artefact was in the harness, not the law.

### 1.4 The slope law, and the defect it exposes

On a plane inclined at θ the tool **centres** lie on a line inclined at θ, so
an XY stepover `d` puts adjacent centres `d·sec θ` apart *along that line* and
the surface-normal cusp is `R − √(R² − (d·sec θ/2)²)`.

| slope | predicted normal cusp | oracle measured | error | vs the 20 µm dial |
|---|---|---|---|---|
| 0° | 0.020000 | 0.020000 | 0.00% | 1.00× |
| 15° | 0.021468 | 0.021137 | 1.54% | 1.06× |
| 30° | 0.026854 | 0.026089 | 2.85% | 1.30× |
| 45° | 0.040870 | 0.040723 | 0.36% | 2.04× |
| 60° | 0.085754 | 0.081887 | 4.51% | 4.09× |

Restated as the rule a candidate must implement: **to hold a constant
surface-normal cusp, the XY stepover must be scaled by `cos θ`.**

`scallop_math::variable_stepover` scales it by `1/√cos θ` — it computes
`R_eff = R / cos θ` and feeds that to the flat formula. It therefore **opens
the stepover on slope exactly where the geometry closes it**:

| slope | geometry requires | `variable_stepover` returns | too wide by |
|---|---|---|---|
| 30° | 0.2425 mm | 0.3013 mm | **1.24×** |
| 45° | 0.1980 mm | 0.3340 mm | **1.69×** |
| 60° | 0.1400 mm | 0.3980 mm | **2.84×** |

Because cusp goes as `d²`, a 1.69× wide stepover is roughly a 3× cusp
overshoot at 45°. This is a **new finding**: it is not the min-across-ring
mechanism the code comments and Checkpoint B's addendum name, and it points
the opposite way (min tightens, the slope term loosens). The two have been
partially cancelling.

At 75° and beyond the closed form saturates — a 0.28 mm XY stepover is 1.08 mm
along a surface a Ø1 ball spans 1.0 mm of. That is a *gap*, not a scallop, and
it is a tool-size regime no stepover law reaches.

---

## 2. Phase B — the arms

`ScallopStepoverPolicy` makes each stacked compensation selectable, so the
arms are a **decomposition**, not a beauty contest.

| arm | what it changes |
|---|---|
| `A0` | shipped |
| `A1` | retire min-across-**polygons** |
| `A2` | retire fixed-20 sampling (every vertex; rings are already decimated at 0.75 × cell, so this *is* the plan's distance-bounded sampling) |
| `A3` | retire min-across-**ring** → p10 |
| `A4` | median-ratio clamp — the plan's item 4, **benchmark only** |
| `A5` | clamp curvature to `|κ| ≤ 1/R` (a ball cannot follow curvature tighter than its own radius) |
| `A6` | corrected slope law (`cos θ`) |
| `A7` | corrected cascade: `cos θ` + κ cap + every vertex + per polygon, MIN kept |
| `A8` | **iso-field** level sets, shipped stepover law |
| `A9` | **iso-field** level sets, corrected stepover law |

Plan item 2 (variable-distance polygon offset) was **assessed and not built** —
see §6.

`A8`/`A9` use `scallop_isofield`: solve `|∇D| = 1/s(x,y)` from the region
boundary by Godunov fast sweeping and take the integer level sets. There is no
per-ring scalar, so min-across-ring and min-across-polygons have nothing to
reduce; there is no repeated offsetting, so the decimation compensation has
nothing to contain; and the ring count is `⌊max D⌋`, known before a ring is
emitted. Both sources feed the **same** 3D lift, chord refinement and
emission, so a comparison across the axis isolates ring placement.

### 2.1 Headline table — generation grid `envelope/4` (0.750 mm), shipped

Surface-normal achieved cusp, ×dial, and the two unfinished-material columns.

| fixture | A0 shipped | A3 no min-ring | A4 median | A7 corrected cascade | A8 iso-field | **A9 iso-field + cos θ** |
|---|---|---|---|---|---|---|
| flat ground | 2.37× | 2.37× | 2.37× | 2.37× | 2.02× | **2.02×** |
| grooved block | 6.28× | 6.32× | 6.32× | 6.03× | 3.78× | **2.86×** |
| narrow ridge | 11.58× | 11.55× | 13.51× | **194.50×** | 10.86× | **4.90×** |
| mixed-slope ribbon | 28.21× | 26.97× | 29.62× | **210.79×** | 26.91× | **26.28×** |
| dome | 3.95× | 4.00× | 4.06× | 3.79× | 3.60× | **2.66×** |
| terrain (20 mm crop) | 23.53× | 22.64× | 24.65× | 89.45× | 25.87× | 25.01× |

Standing material, mm² (`residual > 5 × dial`):

| fixture | A0 | A3 | A4 | A7 | A8 | **A9** |
|---|---|---|---|---|---|---|
| grooved block | 19.87 | 20.01 | 19.98 | 18.56 | 12.12 | **3.90** |
| narrow ridge | 11.05 | 10.76 | 13.70 | 26.21 | 16.77 | **3.55** |
| mixed-slope ribbon | 31.69 | 31.95 | 40.26 | 38.90 | 27.73 | **23.72** |
| dome | 5.64 | 5.66 | 5.75 | 5.24 | 4.13 | **2.14** |

Untouched material, mm² (nothing ever reached it) — zero for every arm on
every fixture **except**:

| fixture | A6 | A7 |
|---|---|---|
| narrow ridge | 38.19 | **40.70** |
| mixed-slope ribbon | 102.01 | **94.87** |
| terrain | 14.43 | 19.01 |

Generation time is 0.39–1.05 s for every arm on every synthetic fixture; the
iso-field is not slower than the cascade (A9 0.42–1.03 s vs A0 0.40–0.67 s),
because the Eikonal sweep is cheap next to the per-ring-point drop-cutter
queries both share.

---

## 3. Findings

### 3.1 min-across-ring explains the *ring count*, and almost none of the cusp

The decomposition run reports `sample p50 / selected`, the factor by which the
reducer slows the cascade below what the typical point on the ring allows.

| fixture | collapse ratio (A0) | rings A0 → A3 → A4 |
|---|---|---|
| flat ground | 1.00× | 27 → 27 → 27 |
| dome | 1.01× | 28 → 28 → 27 |
| grooved block | 1.07× | 47 → 44 → 44 |
| narrow ridge | **1.37×** | 38 → 30 → 26 |
| mixed-slope ribbon | **1.68×** | 38 → 30 → 24 |

So min-across-ring costs up to 1.68× in stepover and up to **37% of the ring
count** — real time. But retiring it moves quality essentially not at all:
narrow ridge 231.7 → 231.0 µm, ribbon 564.2 → 539.4 µm, and the median clamp
makes both *worse* (270.2 and 592.3 µm). Standing material improves modestly
on the ridge (23.0 → 17.0 mm² gouge area; standing 11.05 → 10.76 mm²) and
degrades on the ribbon (31.69 → 40.26 mm² under the median clamp).

**This contradicts Checkpoint B's addendum**, which closed *"the fix is in
`ring_stepover`, not in the budget."* Measured: the fix is in **neither**. The
cusp overshoot is 2.37× on flat ground and 3.95× on a smooth dome, where the
collapse ratio is 1.00× and 1.01× and min-across-ring is costing nothing at
all. A mechanism that is inactive cannot be the cause.

The addendum's recommendation was reasonable on its evidence — it had ring
counts and uncut area, and min-across-ring does drive both. It did not have a
cusp measurement that could be trusted on sloped ground, which is exactly what
phase A was for.

### 3.2 min-across-polygons is unfalsified, not exonerated

`A1` produces a **byte-identical fingerprint to `A0` on all five synthetic
fixtures and on terrain**. None of them ever has two live polygons at an
iteration where the distances would differ. The second minimum is real in the
code and this fixture set cannot see it; a region-scoped scallop on a
dendritic mid-steep band (the P2.c shape) would be needed. Filed, not fixed.

### 3.3 fixed-20 sampling is nearly free

`A2` moves the achieved cusp by ≤ 1 µm on four of five fixtures and adds 0–3
rings. Ring polygons are already decimated at `0.75 × cell`, so the fixed-20
budget rarely misses an extremum the grid can resolve. The plan's fix-sequence
item 1 is therefore **cheap and low-value**, not a lever.

### 3.4 The corrected slope law cannot be adopted into the cascade alone

`A6`/`A7` are the worst arms in the study: 194–211× the dial, 38–102 mm²
untouched, 67–146 mm² of `uncut_core`. The mechanism is unambiguous — both
report **51 rings, exactly `max_rings`** — and the residual map
(`resid_narrow_ridge_A7.png`) shows a 6.4 mm square hole of never-touched
material in the middle of the part, ringed by standing material.

The corrected law is *tighter* on slope (collapse ratio 3.54× on the ridge,
6.43× on the ribbon), and `max_rings` is budgeted from the **flat-ground**
stepover. Making the stepover correct makes the truncation catastrophic.

**The stepover law and the ring budget are not separable.** Any fix that
corrects one without removing the other's cap makes the product worse than
shipped. This also explains, retroactively, v3's "+92% time, 34× over-cut"
result from raising the cap naively: cap and law are one problem.

### 3.5 On flat ground at `envelope/4`, nobody hits the dial

Every cascade arm reads **2.37×** the dial on flat ground; the iso-field reads
2.02×. At `cusp/4` **every one of them reads exactly 1.00× (20.0 µm) with zero
standing material.**

The residual map (`resid_flat_ground_A0.png`) shows why: regular concentric
cusp ridges at the right spacing, plus bright over-dial patches at **the
corners** of every ring and at the collapsing centre. Ring decimation at
`0.75 × 0.750 = 0.5625 mm` rounds the rectangle's corners, and the diagonal
gap between two consecutive corner points is `d√2`, not `d`. The iso-field's
level sets round the corners correctly (`resid_flat_ground_A9.png` shows
visibly rounded rings) and still read 2.02×, because its own ring placement is
quantised to the same 0.750 mm field cells.

So on flat ground the entire overshoot is **ring-placement quantisation at the
generation resolution** — not the stepover law, not the reducer. That is a
finding *for* moving scallop to a finer grid, which is what Checkpoint B
wanted and had to refuse.

### 3.6 Checkpoint B's `cusp/4` rejection does not survive — for the iso-field

Checkpoint B rejected `cusp/4` because the cascade left 19–33 mm² standing
there that `envelope/4` did not. **The harness reproduces that number exactly:
narrow ridge, A0, cusp/4 → `uncut_core` 19.32 mm²**, bit-identical to the
Checkpoint B figure, which is an independent confirmation that the two packs
are measuring the same thing.

| fixture | arm | envelope/4 → cusp/4: uncut core | standing mm² | cusp ×dial |
|---|---|---|---|---|
| flat ground | A0 | 0.00 → 0.00 | 0.14 → **0.00** | 2.37× → **1.00×** |
| flat ground | A9 | 0.00 → 0.00 | 0.33 → **0.00** | 2.02× → **1.00×** |
| grooved block | A0 | 0.00 → 0.00 | 19.87 → 12.01 | 6.28× → 4.56× |
| grooved block | A7 | 0.00 → **15.38** | 18.56 → 17.22 | 6.03× → 6.22× |
| grooved block | A9 | 0.00 → 0.00 | 3.90 → **2.00** | 2.86× → **1.87×** |
| narrow ridge | A0 | 0.00 → **19.32** | 11.05 → 12.59 | 11.58× → **88.63×** |
| narrow ridge | A7 | 70.38 → **126.52** | 26.21 → 29.64 | 194.50× → 210.79× |
| narrow ridge | A9 | 0.00 → **0.00** | 3.55 → **1.46** | 4.90× → **3.42×** |
| ribbon | A0 | 0.00 → 0.00 | 31.69 → 26.37 | 28.21× → 26.24× |
| ribbon | A9 | 0.00 → 0.00 | 23.72 → **21.89** | 26.28× → 26.24× |
| dome | A0 | 0.00 → 0.00 | 5.64 → 5.16 | 3.95× → 3.77× |
| dome | A9 | 0.00 → 0.00 | 2.14 → 5.06 | 2.66× → 3.75× |

**The rejection is a property of the offset cascade, not of the resolution.**
Under the cascade it survives and the "culprit fix" makes it far worse
(126.52 mm² uncut on the ridge). Under the iso-field it evaporates: zero uncut
core everywhere, standing material *down* on four of five fixtures, and cusp
improved on four of five. The dome is the single regression (2.66× → 3.75×).

### 3.7 Terrain cannot adjudicate this question, and says so with a number

Every arm lands within 4 µm of every other on the 20 mm terrain crop (22.6–
25.9× dial). The **tool-reach floor** — the residual an infinitely dense path
with this cutter would still leave — is **p99 425.9 µm**, against a best arm of
395.9 µm and a shipped arm of 470.6 µm.

Nearly the whole terrain residual is **geometry no ring placement controls**: a
Ø1 tip cannot enter relief narrower than 0.5 mm and terrain has a great deal
of it. Without that floor this harness would have ranked candidates on a
number none of them moves — the same trap the v3 campaign hit with the same
fixture.

The *ordering* is still readable as a relative signal: at `cusp/4`, A9 improves
(500.2 → 395.9 µm, on-dial 15.9% → 25.7%, standing 159.2 → 107.5 mm²) while A0
degrades badly (470.6 → 1587.8 µm, rings 52 → 120).

### 3.8 What the iso-field costs

Not free, and the fix phase must handle these:

* **Deeper isolated gouge.** On the grooved block the deepest single point is
  −1115 µm (A8) / −995 µm (A9) against the cascade's −108.6 µm. Gouge *area*
  is better (2.40 / 5.01 mm² vs 12.29 mm²), so the defect is localised, not
  systemic — a contour crossing a groove rim where the cascade's rings ran
  parallel to it. This is the candidate's main open risk.
* **Short segments.** Minimum emitted segment 0.0008–0.0055 mm vs the
  cascade's 0.040–0.200 mm, and `p01` 0.024–0.083 mm vs 0.280 mm. The
  population is small — **0.0–0.5% of segments below 10 µm** (worst case 32
  segments on the grooved block) — so M4's junction-cost gate is *passable*
  but needs a decimation pass, which the cascade already has and the iso-field
  currently does not.
* **Ring placement is grid-quantised**, where an offset ring is exact in XY.
  §3.5 is that cost showing up on flat ground.
* A saddle in `D` merges or splits level sets silently. Correct for coverage,
  a hazard for anything downstream assuming ring identity.

---

## 4. The `variable_stepover` slope term is a defect in its own right

Independent of which ring source wins, `scallop_math::variable_stepover` is
inverted in slope (§1.4). It is consumed by `scallop.rs` only. Two
consequences worth stating separately from the algorithm choice:

1. Checkpoint B measured scallop's achieved cusp at "138–157% of the dial" on
   its fixtures and read `cusp/4` bringing it "down to on-dial". This study
   suggests that improvement was partly **accidental**: a finer grid raises the
   raw curvature estimate (`κ` scales as `1/cell²`), which tightens the
   stepover and masks the loosening from the inverted slope term. Two errors
   partially cancelling is not a working dial.
2. The correction is a three-line change with a ground-truth test behind it —
   but §3.4 shows it must **not** be landed on its own.

---

## 5. Against M4's stated acceptance gates

| gate | A9 (iso-field + cos θ) | verdict |
|---|---|---|
| No truncation on supported synthetic and real fixtures | `uncut_core` 0.00 and untouched 0.00 mm² on all 6 fixtures at both resolutions | **met** |
| Achieved cusp within tolerance on every slope class | improved vs shipped in every band on every fixture; but **absolute** cusp is still 2.0–4.9× the dial (and 26× on the ribbon's 85° band, which is tool-limited) | **not met in absolute terms** — see §7 |
| No increase in deep over-cut or collisions | gouge *area* down (2.1–5.0 vs 2.3–12.3 mm²); deepest single point **up 10×** on the grooved block | **partially met** |
| Flat segments not forced to the steep segment's minimum stepover | by construction — no per-ring scalar exists | **met** |
| Runtime improves against uncapped-min and does not regress shipped | 0.42–1.03 s vs shipped 0.40–0.67 s; ring counts comparable | **met** |
| Min segment-length distribution compatible with accel/junction limits | 0.0–0.5% of segments < 10 µm; `p01` 4–10× shorter than the cascade | **met with a caveat** — needs decimation |
| Continuous and discrete modes both covered | only discrete measured in this wave | **not covered** |

---

## 6. Rejected and deferred options, with reasons

**Plan item 2 — variable-distance polygon offset. Assessed, not built.**
`polygon::offset_polygon` takes one scalar distance and has no per-vertex mode;
building one means a self-intersection-repair pass, which is a small offset
engine. That file is **M5's** item (Checkpoint D) and already carries a known
vertex-inflation defect that scallop's decimation exists to contain — building
a second offset path on top of an unfixed one would be stacking a compensation
in the same wave that is supposed to stop doing that. The iso-field reaches the
same objective (per-location stepover) without touching `offset_polygon` at
all, so item 2 is **superseded rather than merely deferred** if the iso-field
is adopted.

**Plan item 4 — median-ratio clamp (`A4`). Rejected.** It is the fastest arm
(24–27 rings on the ribbon vs 38) and it is *worse on quality on every
fixture*: cusp 13.51× vs 11.58× on the ridge, 29.62× vs 28.21× on the ribbon,
standing material 13.70 vs 11.05 mm² and 40.26 vs 31.69 mm². It buys time by
knowingly violating the cusp on the tighter half of each ring, and the oracle
now quantifies the price. The plan asked for it as a benchmark; it has been
benchmarked and should not be adopted.

**`ReachPolicyStepover` ring budget (Checkpoint B PR-8c).** Already rejected
there (recovered a third of the defect). This study explains why no budget
derived from a *nominal* stepover could work: §3.4 shows the budget and the
stepover law are one problem, and §3.6 shows the iso-field removes the budget
entirely rather than resizing it.

**Retiring min-across-ring on its own (`A3`).** Not rejected, but demoted: it
is a **time** optimisation worth up to 37% of ring count, not a quality fix
(§3.1). It should be sequenced as such, and not sold as the repair.

---

## 7. Recommendation

**Adopt the iso-field ring source (`A9`) as the target architecture, subject
to a COLUMNS-gated end-to-end A/B, and land it as one change with the
corrected slope law and the removal of `max_rings`.**

The reasoning, in the order the evidence forces:

1. The compensations M4 set out to remove are **not the binding defect**.
   Min-across-ring costs time and not quality (§3.1); min-across-polygons is
   invisible on every fixture (§3.2); fixed-20 sampling is nearly free (§3.3).
   A repair programme aimed at them would have moved little.
2. The binding defects are an **inverted slope law** (§1.4) and a **ring
   budget that cannot survive fixing it** (§3.4). They are one problem.
3. The offset cascade cannot hold a correct stepover law, because a correct
   law is tighter and the cap is sized from the flat-ground stepover. The
   iso-field has no cap to fix — its termination is `⌊max D⌋`, exactly M4's
   fix-sequence item 4.
4. `A9` is better than shipped on **every fixture** on cusp and standing
   material, at comparable time, with zero truncation at both resolutions.
5. It **unblocks Checkpoint B's held decision**: under `A9`, `cusp/4` becomes
   a straight improvement rather than a trade, and flat ground hits the dial
   exactly (§3.5, §3.6).

**Honest limits on that recommendation.** `A9` does not bring the absolute
cusp to the dial (2.0–4.9× on the synthetic set); §3.5 says the remainder at
`envelope/4` is ring-placement quantisation, and the `cusp/4` rows support
that, but this study has not demonstrated an arm that hits the dial on sloped
ground. It also has a localised 10× deeper gouge on one fixture (§3.8) that
must be understood before it ships, and continuous mode is unmeasured.

---

## 8. Checkpoint C decision menu

**What is being decided:** which architecture M4's implementation phase
targets, and whether the `variable_stepover` slope correction is landed
separately or as part of it. Nothing here has changed production; the
implementation phase is the next wave and step 8 of the sequencing checklist
stays unticked until it lands.

| # | option | consequence |
|---|---|---|
| **1** | **Adopt the iso-field, gated** *(recommended)*. Implementation wave lands `scallop_isofield` as the ring source together with the `cos θ` law and the removal of `max_rings`, behind an end-to-end wanaka-class COLUMNS A/B as the decisive gate — the shape of the M3 ruling. Falls back to the cascade if COLUMNS regresses. | Best measured quality at equal time; removes three compensations and the cap at once; unblocks Checkpoint B's `cusp/4` hold. Largest blast radius in this programme so far: it changes ring topology, so anything downstream assuming ring identity must be checked. Must resolve the localised gouge (§3.8) and add a decimation pass. |
| **2** | **Fix the slope law only.** Land `StepoverGeometry::CosineSlope` in the cascade and nothing else. | **Actively harmful — measured.** 194–211× dial, 38–102 mm² untouched (§3.4). Only viable if `max_rings` is removed in the same change, which the cascade cannot currently do safely. Not recommended in any form. |
| **3** | **Take the time win only.** Land `A3` (retire min-across-ring), leave the law and the cap alone. | Up to 37% fewer rings on sloped fixtures at unchanged quality (±3%). Cheap, low-risk, honest — but it is a *performance* change mis-labelled as the M4 repair, and it leaves the inverted slope law in place. Reasonable as an interim if option 1 is deferred. |
| **4** | **Defer M4; keep the evidence.** Ship nothing; the seam, oracle and study stand as the record. | Zero risk. The `variable_stepover` inversion stays live and undocumented in the dial's user-facing meaning, and Checkpoint B's `cusp/4` hold stays blocked indefinitely. If chosen, the inversion should at minimum be documented in `scallop_math` so the next reader is not misled. |

Independent of 1–4, two sub-decisions:

* **5a.** Should `scallop_math::variable_stepover`'s inverted slope term be
  **documented in place** now, regardless of which option is taken? It is
  currently described in its own doc comment as correct. *(Recommend: yes,
  unconditionally — it costs nothing and the comment is presently wrong.)*
* **5b.** Should `ScallopReport` gain the **untouched/standing split** the
  oracle uses, so truncation is visible in production telemetry rather than
  only in a research harness? `uncut_core_mm2` is hole-blind and cascade-only.
  *(Recommend: yes, and A/M9's channel is the place.)*

---

## 9. What this study could not do

* **Multi-polygon cascades** — `PolygonReduce` is unfalsified on this fixture
  set (§3.2). Needs a dendritic region-scoped fixture.
* **Continuous mode** — every arm ran discrete.
* **The absolute cusp on slope** — no arm reaches the dial on sloped ground;
  §3.5 attributes the remainder to placement quantisation but does not prove
  it by exhibiting an arm that closes it.
* **Real relief** — terrain is tool-reach-bound (§3.7). A fixture with relief
  coarser than the tool but genuine slope variety is still missing from this
  suite; the synthetic five stand in for it.
* **Wall-clock on a wanaka-scale part** — all timings are 16 mm fixtures at
  sub-second scale. A ranking on generation time at that size does not
  establish one on a real part.
