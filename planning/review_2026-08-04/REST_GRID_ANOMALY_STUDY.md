# Rest-grid refinement anomaly — instrumented study

Wave: W7 (R6/M3) · Date: 2026-08-05 · Parent revision: `ebe77de`
Subject: `crates/rs_cam_core/tests/rest_grid_resolution_c9.rs` and
`crates/rs_cam_core/src/rest_field.rs`

**Scope discipline, stated up front.** Per the plan: *do not change rest-grid
production resolution as a response to the anomaly*, and
*`rest_grid_resolution_c9` remains green until a replacement explains and
supersedes it.* **No production file is touched by this wave. No cargo command
was run.** The three instrumented arms in §7 are **NOT RUN** and carry
pre-registered predictions so they cannot be retro-fitted.

---

## 1. The anomaly as pinned

`rest_grid_resolution_c9.rs` sweeps `RestFieldParams::cell_mm` over
`[0.5, 0.25, 0.1]` with everything else fixed, on two `GroovedBlock` fixtures,
and records (module doc `:23–56`):

- **tip-scale groove** (rim half-width 0.8 mm, 70° walls, 1.2 mm deep): the
  shipped 0.5 mm cell finds one centreline of **19.0 mm**; 0.25 mm and 0.10 mm
  find **nothing at all**.
- **wide control** (rim half-width 2.5 mm): all three cells find both grooves
  (38.0 / 37.0 / 36.4 mm — flat, as a control should be), but median per-point
  reach **collapses**: **0.583 → 0.242 → 0.000 mm**.

The sentry is **not `#[ignore]`d** — both tests run in the normal suite. Its
assertions are deliberately weak and it pins the anomaly *as an anomaly*:

```rust
:215  assert!(fine.centerlines == 0, "the 0.10 mm cell now finds {} centreline(s) …")
:268  assert!(coarse.median_reach_mm > fine.median_reach_mm, …)
```

**Both go red when the anomaly is FIXED.** That is correct sentry design for an
open question, and it means any future repair must expect these two to fail and
must rewrite the module doc rather than delete the lines — the assertion
messages say so.

---

## 2. The first finding: this is two anomalies, not one

They have been reported as one headline ("refinement finds less"). The pipeline
says they are separate phenomena with separate mechanisms:

| | tip-scale groove | wide control |
|---|---|---|
| symptom | centrelines 1 → 0 → 0 | length flat; **reach** 0.583 → 0.242 → 0.000 |
| detection | — | never fails |
| candidate stage | **routing verdict** (§4) | **ridge location + rim quantisation** (§3) |

Conflating them is why the anomaly resisted diagnosis: a mechanism that
explains a reach collapse does not explain a detection loss, and vice versa.

---

## 3. Where `h` enters, and which entries invert

`detect_rest_valleys` (`rest_field.rs:616`) is nineteen stages. Every stage
where the cell size `h` enters, and its direction under refinement:

| stage | site | `h` enters as | inverts? |
|---|---|---|---|
| grid extent / **lattice phase** | `:630–636` | `margin_cells = ceil(radius/cell)+1`; `origin = bbox.min − margin_cells·cell` | **non-monotone** — which physical x's get sampled changes with `h` |
| field sampling | `:640–695` | point drop-cutter at `origin + c·cell`; **no area averaging** | — |
| threshold mask | `:717–727` | `rest > 0.05` — **absolute mm against a point sample** | — |
| components | `:759–824` | `MIN_REGION_CELLS = 4` — **fixed cell count**, area `4h²` | coarse drops more (expected direction) |
| **box smooth** | `:1348–1349` | **fixed 3×3 window** → physical width `3h` (1.5 mm at h=0.5, 0.3 mm at h=0.1) | **YES — §3.1** |
| **NMS prominence** | `:1417–1418`, `:1444` | `max(0.1·mvd, 0.005 mm)` — **absolute mm** vs a **one-cell** difference | **YES — §3.4** |
| hysteresis | `:1503` | `peak ≥ min_valley_depth` — absolute mm on a point sample | — |
| graph cleanup | `:867` | `min_cut_length/cell` compared in cell units | no — scale-consistent |
| branch saliency | `:896` | `median(branch_rest) < 0.05` | second-order |
| **cross-section walk** | `:549`, `:573`, `:592` | `CROSS_SECTION_WALK_CELLS = 64` **cells**; `rim_cells = j as f64`; `dist = rim_cells · cell` | **YES — §3.2, §3.5** |
| wall slope | `:580` | `max_slope = max(rise/cell)` — single-cell secant | **YES, opposite sign — §3.3** |
| depth | `:538` | `depth = rest.at_index_or(ridge, 0.0)` — **point sample of a maximum** | **YES — §3.1** |
| reach solve | `reach.rs:353` | `X = rim_distance − profile_rise(cot θ, δ)`; scan step `0.002 mm` **absolute** | no `h` at all |

**"Interpolation" does not exist on this path.** Every stage is nearest-cell;
there is no sub-cell refinement of the rim crossing, the ridge position, or the
peak. That rules out one of the plan's five named candidate stages outright.

The structural fact that organises everything below:

```
reach X = rim_distance(h)  −  profile_rise(cutter, cot θ(h), δ(h))
          └─ biased HIGH at coarse h ─┘   └── essentially h-independent ──┘
```

**Any positive bias in `rim_distance` at coarse `h` converts one-for-one into
manufactured reach, and refinement removes it.** The reach collapse does not
need a mechanism that destroys reach; it needs a mechanism that *manufactured*
it at 0.5 mm.

### 3.1 Suspect 1 — the ridge LOCATION migrates with `h` (leading, control)

`box_smooth_rest` averages over a window of **physical** width `3h`
(`:1348–1349`); `nms_candidates` then picks the argmax by comparing
single-cell neighbours (`:1444`). On a rest peak that is asymmetric — gentle
inboard, cliff outboard, which is exactly a groove — a box mean of width `3h`
shifts the smoothed argmax **toward the gentle side by O(h)**.
`measure_cross_section` then walks from *that* cell, so an inboard ridge
inflates the rim distance by that offset.

Compounding: `depth` (`:538`) is a **point sample of a maximum**, so a coarse
lattice systematically *under-reads* the peak → smaller `δ` → smaller
`profile_rise` → **larger** reach. The coarse grid mislocates the ridge *and*
under-reads its depth, and **both errors push reach up**.

A hand derivation (arithmetic only, **NOT executed** — this is the study's
central hypothesis, not a result) puts the h=0.5 ridge about **0.7 mm inboard**
of the true rest maximum on the wide control, i.e. **1.4 cells**, and predicts
reach ≈ 0.58 / 0.26 / ≤ 0 across the sweep against the logged
0.583 / 0.242 / 0.000. If that reproduction survives execution it is decisive;
until it is executed it is a prediction, and §7 arm 2 is written to falsify it.

**Note this revises the module doc's own hypothesis.** `:38–41` blames
`rim_distance = rim_cells × cell` quantisation. That is real (§3.2) but bounded
by one cell — at most 0.5 mm, and on average 0.25 mm. It is **the smaller of
the two mechanisms**, and it cannot by itself move reach by 0.583 mm.

### 3.2 Suspect 2 — `rim_distance` is an integer cell count, rounded up

```rust
:573   rim_cells = j as f64;
:592   let dist = rim_cells * cell;
```

`rim_cells` is the index of the **first sampled cell at which
`rest ≤ threshold`**, so the reported rim lies between the true crossing and
one full cell beyond it: bias `+U(0, h)`, with a hard floor of `h` even for a
rim 0.01 mm away. Real, directional, bounded by one cell.

### 3.3 Suspect 3 — `max_slope` is a single-cell secant, and it inverts the *other* way

`max_slope = max(rise/cell)` (`:580`) is a secant over `h`. On a convex
tool-drop profile it **under-states** the true gradient at coarse `h` → larger
`cot θ` → larger `profile_rise` → *smaller* reach. Listed explicitly so the
study does not mis-attribute: this term **partially cancels** suspects 1, 2 and
the depth error, which is why the observed curve is not a clean `X ∝ h`.

### 3.4 Suspect 4 — an absolute-mm prominence against a one-cell difference

`prominence = max(0.1·min_valley_depth, NMS_PROMINENCE_FLOOR_MM)` with the
floor at **0.005 mm absolute** (`:1382`, `:1418`), compared against a
difference measured over **one cell** of a smoothed field (`:1444`). For a
smooth maximum that difference scales as `½|f″|h²`; against a constant bar it
**vanishes under refinement**, so a fine grid loses ridges a coarse one keeps.

This is the mechanism the anomaly's headline would naively be blamed on — and
on *these two fixtures it is probably not what happens*, because a groove's
rest peak is a kink, not a smooth dome, so its one-cell prominence grows with
refinement. **It should be ranked much higher for any fixture with a broad,
smooth rest maximum**, which most real parts have. It is a latent defect this
study found while looking for a different one.

### 3.5 Suspect 5 — a walk budget fixed in CELLS

`CROSS_SECTION_WALK_CELLS: usize = 64` (`:398`). Physical budget = **32 mm at
h=0.5, 6.4 mm at h=0.1, 1.6 mm at h=0.025**. The doc at `:393–397` reasons
about it in mm at the shipped cell and says so out loud. When the budget runs
out, `rim_cells` stops at 64 and the side reports `dist = 64h` **as though a
rim had been found — there is no flag distinguishing "found a rim" from "gave
up"**. Not binding on these two fixtures, but it binds below ≈ 0.05 mm and it
is why this sweep cannot be joined to `checkpoint_a_valley_matrix`'s
`C9_PITCHES` (down to 0.001 mm) without changing the constant.

---

## 4. The headline finding: "finds nothing" is probably "found it and refused it"

`rest_grid_resolution_c9.rs` counts `rf.centerlines`. **That is a
post-routing list.** Verified directly at `rest_field.rs:951` and `:970–991`:

```rust
:951        skeleton_length += len;            // BEFORE routing
:969        let verdict = branch_verdict(pencil, &samples, offset_stepover, params);
:970        if verdict == RoutingVerdict::Pencil {
:982            centerlines.push(RestCenterline { … });
:986        } else if comp != usize::MAX {
:991            clearing_comps.insert(comp);   // Clearing AND Refused land here
```

**A branch that is detected, traced, measured, and then refused never appears
in `centerlines`.** The sentry therefore cannot distinguish *"not detected"*
from *"detected and refused"* — and it reads neither
`rf.report.skeleton_length_mm` (accumulated pre-routing) nor
`rf.clearing_regions`, both of which already carry the distinction.

The mechanism that would produce refusal at fine `h` is the same §3 arithmetic:
`solve_reach` sets `refused: true` when `x_left + x_right < 0`
(`reach.rs:370`), and §3 predicts exactly that both sides go negative once the
manufactured rim distance is removed.

**Pre-registered prediction, falsifiable with ZERO code change:** at h = 0.1 on
the tip-scale groove,

> `rf.report.skeleton_length_mm ≈ 19 mm` **and** `rf.clearing_regions.len() ≥ 1`
> **while** `rf.centerlines.is_empty()`.

If that holds, **the anomaly's headline is wrong**: refinement does not find
less feature, it *routes* the same feature differently, and every downstream
statement of the form "the fine grid detects nothing" must be restated.

One caveat found while verifying this: `clearing_comps.insert(comp)` is guarded
by `comp != usize::MAX` (`:986`). A refused branch whose ridge cells all sit
outside the threshold mask lands in **neither** list and disappears without
trace. That is a reporting blind spot in its own right, independent of the
anomaly.

### 4.1 The dials make routing a refusal detector

With `num_offset_passes_cap: 8` and `offset_stepover_mm: 0.25`, the
Pencil/Clearing threshold is `8 × 0.25 = 2.0 mm` of reach — which neither
fixture's narrow side approaches. **On these fixtures the only reachable
non-Pencil exit is `Refused`.** Any study must state this; the sentry's dials
turn a three-way router into a binary refusal detector.

---

## 5. Stage of death — named

The plan asks W7 to *"name the stage where the signal dies"*. Two stages, one
per phenomenon, with the evidence class for each:

| phenomenon | stage where the signal dies | evidence class |
|---|---|---|
| **control reach collapse** | **stage 8+9, ridge extraction** — `box_smooth_rest` (`:1326–1363`) followed by `nms_candidates` (`:1408–1454`) place the ridge O(h) inboard of the true rest maximum; `measure_cross_section` then walks from the wrong cell and `rim_distance` inherits the error one-for-one | code-structural (certain) + hand arithmetic (unverified) |
| **tip-scale "detection" loss** | **stage 19, `branch_verdict` / `solve_reach`** — the branch is found and then refused; the signal dies in *routing*, not in detection | code-structural (**verified**, §4) + prediction (unrun) |

Neither stage is "mask threshold" or "interpolation". The module doc's named
suspect, rim quantisation, is a real but **secondary** contributor.

---

## 6. Instrumentation seams

**No hook is needed for the decisive arm.** Already public:

| quantity | seam |
|---|---|
| ridge geometry | `RestCenterline::points: Vec<P3>` (`:306`) — world XY at cell centres; **the direct instrument for §3.1** |
| per-point cross-section | `CenterlineSample { valley, reach }` (`:352–358`), all pub. `valley.left.rim_distance_mm / cell` **recovers `rim_cells` exactly**; `wall_rise_mm == 0.0` flags the vertical-wall fallback |
| found vs routed away | `report.skeleton_length_mm` (`:186`, pre-routing) vs Σ `centerlines` length (`:975`, post) — **§4's whole prediction, zero new code** |
| refused/cleared | `RestFieldResult::clearing_regions` (`:369`), `report.clearing_region_count` |
| the field itself | `RestFieldResult::rest_grid: RestGrid` (`:225–239`) — pub `nx, ny, origin, cell_mm, rest, surface_z, threshold`. Caveats: untrusted cells are `NaN` here but not in the internal grids, and it is `f32` vs internal `f64` |
| re-solving reach | `reach::solve_reach`, `profile_rise`, `route` — all pub, so a test can perturb `rim_distance_mm` by ±h on a **detector-produced** valley and re-solve, isolating rim sensitivity without touching the detector |
| **rendering** | `mod hillshade` (`:1809–1938`, `#[cfg(test)] pub(crate)`) already renders the rest field with ridge overlays to PNG; driver `render_restfield_hillshade` (`:2637`) |

**Hook needed only for the deeper arms**: stages 8–17 are private free
functions (`box_smooth_rest`, `nms_candidates`, `hysteresis_ridge`,
`zhang_suen_thin`, `trace_skeleton`, `cleanup_ridge_graph`,
`ridge_perpendicular`, `measure_cross_section`, `past_rim_cells`,
`branch_verdict`). `rest_field.rs`'s own `#[cfg(test)] mod tests` (`:1942`) can
call them via `use super::*` — and **already does** for several
(`chamfer_dt_center_of_band` `:2492`, `thinning_reduces_a_thick_line_to_one_cell`
`:2511`, `trace_recovers_a_straight_skeleton` `:2532`). An integration test
under `tests/` cannot. **So the ridge-displacement arm belongs in the unit
suite, not in `tests/`** — no API change, and there is precedent.

---

## 7. The study, cheapest first — NOT RUN

Ordered so the cheapest arm can falsify the most.

**Arm 1 — zero-code routing probe (integration).** Re-run the two c9 fixtures
reading `report.skeleton_length_mm`, `report.clearing_region_count`,
`clearing_regions`, and per-sample `valley.left/right.rim_distance_mm`,
`.wall_rise_mm`, `valley.rest_depth_mm`, `reach.refused`.
**Pre-registered prediction: §4** — detected and refused, not undetected.
*Falsified if* `skeleton_length_mm ≈ 0` at h = 0.1, which would move the stage
of death back to ridge extraction for both phenomena.

**Arm 2 — ridge-position probe (unit, inside `rest_field.rs::tests`).** Build a
synthetic `Grid2<f64>` rest field with a known analytic asymmetric peak; run
`box_smooth_rest` + `nms_candidates` at **≥ 8 cell sizes** and record the
argmax displacement from the analytic maximum.
**Pre-registered prediction: displacement ∝ h, ≈ 0.7 mm at h = 0.5** on the
control's profile. *Falsified if* displacement is `O(h²)` or unbiased.
**Eight sizes, not three** — §3's lattice-phase entry makes a 3-point sweep
capable of showing a trend that is really aliasing.

**Arm 3 — rim-sensitivity probe (unit, public API only).** Take a
detector-produced `LocalValley`, perturb `rim_distance_mm` by ±h, re-solve with
`solve_reach`. **Prediction: `dX/d(rim) = 1` exactly** — reach inherits rim
error one-for-one. This is a contract check, and it should be a permanent
sentry regardless of the anomaly's outcome.

**Arm 4 — render.** `mod hillshade` PNG with ridge overlay at each `h`, read
before any verdict. Standing rule: never gate on an aggregate without rendering
the surface.

**Not to be done as a convenience:** raising `CROSS_SECTION_WALK_CELLS`. It is
32 mm at the shipped cell, and raising it silently changes what "found a rim"
means.

---

## 8. What the study must not change

All three production defaults are the literal `0.5` written in **three**
places — itself worth a note:

| site | value |
|---|---|
| `rest_field.rs:151` `RestFieldParams::default().cell_mm` | 0.5 |
| `pencil.rs:515–517` `rest_cell_default()` | 0.5 |
| `compute/config.rs:1332` `RestAnalysisConfig::default().cell_mm` | 0.5 |
| GUI pencil dial `viz/ui/properties/operations/surface_3d.rs:415–427` | range **0.1–2.0** mm |
| GUI rest-analysis dial `viz/ui/properties/mod.rs:4166–4176` | range 0.05–10.0 mm |

**`auto_resolution` is NOT the rest grid** — it is the dexel simulation cell
(`session/compute.rs:2111`, `viz/controller/events/simulation.rs:234`). Nothing
derives the rest cell from the tool, and the C9 record is explicit that
deriving it the way `FinishResolutionPolicy` does would not help, because the
pitch the sampled model needs is two to three orders **below** the tip radius.

Note the GUI pencil dial's floor is **0.1 mm** — the fine end of this sweep is
reachable by a user today, so whatever the fine arm does is shipped behaviour,
not a laboratory curiosity.

---

## 9. The dark code, and why it is adjacent but not implicated

`reach::solve_reach_sampled` (`reach.rs:524–599`) + `SampledCrossSection` are
reached only if `PRODUCTION_REACH_MODEL` (`reach.rs:649`) is flipped off
`WallAngleV`. **The c9 sentry does not exercise it** — every number in that
file, including 0.583 → 0.242 → 0.000, is the **shipped** path.
`measure_cross_section` nevertheless *computes* the sampled section on every
point (`:607`, and the whole `owed`/`past_rim` machinery exists to feed it) and
then discards it.

The 0.002–0.010 mm pitch requirement **is documented in code**, in full, with
its table, at `reach.rs:601–649` — *"flip this constant when the rest cell is
fine enough, and not before."*

Bearing on the anomaly: none directly, but it sharpens why the anomaly matters.
C9's "leave `cell_mm` at 0.5" rested partly on the replacement model being
unadoptable. This study says the **shipped** model's reading is also
`h`-unstable — an independent argument about the same dial, and one that does
not depend on the dark model ever being adopted.

---

## 10. Evidence-based next owner

**Owner: whoever next touches `rest_field.rs`'s ridge-extraction stages**
(`box_smooth_rest` / `nms_candidates`), *not* `measure_cross_section`.

This is a change from the standing assignment. `ANTIPATTERNS_BACKLOG.md:188`
and `ORCHESTRATION_LOG.md:4909` both name *"whoever next touches
`rest_field::measure_cross_section`"*. On this evidence
`measure_cross_section` is **downstream of the defect**: it walks correctly
from the cell it is given, and its own contribution (rim quantisation,
bounded by one cell) is the smaller mechanism. Re-pointing the ownership is
one of this study's deliverables.

**Re-open condition:** Arm 1 executed. It is cheap, it needs no code change,
and it can invalidate the anomaly's headline outright.

**Sentry status: `rest_grid_resolution_c9` stays exactly as it is.** It is
green, it is not `#[ignore]`d, and it pins the anomaly in the correct
direction. Nothing in this document asks for it to move.

---

## 11. Asks at Checkpoint E

- **E9.** Approve Arms 1–4 as a scheduled, non-production instrumented study,
  with Arm 1 first and its prediction recorded as above?
- **E10.** Approve re-pointing the anomaly's owner from `measure_cross_section`
  to the ridge-extraction stages, and correcting the two backlog entries?
- **E11.** Note two defects found *incidentally* and not part of the anomaly:
  (a) the absolute-mm NMS prominence floor against a one-cell difference
  (§3.4) — latent, and it bites hardest on broad smooth rest maxima, i.e. real
  parts; (b) a refused branch whose ridge cells all fall outside the threshold
  mask is dropped from **both** `centerlines` and `clearing_regions` (§4).
  Neither is fixed here.
