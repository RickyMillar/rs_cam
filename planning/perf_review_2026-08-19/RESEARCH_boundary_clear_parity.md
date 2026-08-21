# RESEARCH — planner/sim parity with a `ToolContainment` boundary set

**Lane:** W7 research (read/test-only), 2026-08-21. Branch `tech-debt-3`,
tip `1d6dd855`. Nothing committed; the scratch instrument was deleted after
measurement (source preserved under **Artifacts**).

**Closes OPEN ITEM 1 of `DELTA_w5b_f1_planner_boundary.md` §7(a)** — "the
`boundary` polygon clear is the production analogue, and is un-measured".

---

## 0. Verdict in one paragraph

Setting a containment boundary on adaptive3d takes whole-grid `sim_higher`
(the planner claiming removal its emitted path does not deliver) from **0 to
5090 cells on ContourParallel and 0 to 5065 on AgentSearch**, on the same
fixture, with one parameter changed. §7(a)'s prediction is **confirmed in
direction** — but it named the *smaller* of two mechanisms. 4005 of those
cells are outside the containment, are physically unreachable by a compliant
tool, and are a labelling artifact with no consumer. The consequential term is
the other **1085 / 1060 cells**, which are **inside** the containment,
tool-reachable, standing up to **24.5 mm** deep, and produced entirely by the
**post-generation clip** — not by the pre-clear. On top of that, **138 emitted
cutting moves per run reach fully outside the containment** on *both*
strategies, from the boundary-unaware `waterline_cleanup`. **No downstream
consumer reads the planner's claim grid**, so rest machining is unaffected and
severity is bounded: safety LOW, machining-quality MEDIUM, instrument-blindness
MEDIUM-HIGH.

---

## 1. The fixture

A copy-and-extend of `adaptive3d::tests::run_planner_sim_parity_with_mesh`,
rebuilt on the public API so it could live in `tests/` without touching a
tracked file (`adaptive_3d_segments` and `segments_to_toolpath` are
`pub(super)`). Planner state comes from the public
`debug_adaptive_3d_segments_for_f029_probe` (which re-runs the planner with the
same params — deterministic), the toolpath from
`adaptive_3d_toolpath_with_cancel`.

| | |
|---|---|
| mesh | `make_test_hemisphere(20.0, 16)` — the existing parity fixture's mesh |
| cutter | `FlatEndmill::new(6.35, 25.0)`, radius `r` = 3.175 mm |
| stock / grid | `mesh.bbox ± r` = [-23.175, 23.175]², z 0..25, cell 0.5292 mm → **89 × 89 = 7921 cells** |
| `world_stock_xy_bbox` | `Some((-1e4, -1e4, 1e4, 1e4))` — the **shipped** configuration, so the F-027 border clear is fully inhibited and the boundary clear is the only pre-clear left |
| depth_per_pass / stepover / stock_to_leave / tolerance | 3.0 / 1.0 / 0.5 / 0.5 |
| tolerance for "cells differ" | cell_size = 0.5292 mm (same as the in-lib fixture) |
| **boundary** | `Polygon2::rectangle(-23.5, -23.5, 0.0, 23.5)` — the **left half** of the stock, `ToolContainment::Inside` |
| clip | `effective_boundary_reported(boundary, Inside, r)` → 1 polygon, then `clip_toolpath_to_boundary` — the same two steps `session::compute::apply_boundary_clip` performs |

### 1.1 The instrument reproduces the in-lib fixture exactly

Not asserted — measured. With `boundary: None` the rebuilt harness produces the
in-lib world-stock-declared control's numbers **to the cell and to the move**:

| | in-lib control (`DELTA_w5b_f1` §5) | this harness, `boundary: None` |
|---|---:|---:|
| ContourParallel — divergent / `sim_higher` / max dz / moves | 5 / 0 / 1.747 mm / 3949 | **5 / 0 / 1.747 mm / 3949** |
| AgentSearch — divergent / `sim_higher` / max dz / moves | 81 / 0 / 4.161 mm / 1522 | **81 / 0 / 4.161 mm / 1522** |

> One trap recorded on the way: setting `world_stock_xy_bbox` to the *nominal*
> stock rect instead of a wide box re-admits 177 border-cleared cells and 21
> spurious `sim_higher`. `TriDexelStock::from_stock` rounds the grid extent
> **up**, so the last row/column sit outside the declared world bbox and
> `path.rs`'s `x <= wx_max` test fails on them. Production is not exposed
> (`execute.rs:1600-1605` declares the auto-grown stock bbox, and the
> simulator's grid is bounded by the same box) but any future fixture that
> declares a world footprint must declare it **wider than the grid**.

### 1.2 The pre-clear provably engages

`w7_preclear_engages` compares the planner's own stock outside the boundary
with and without the boundary set, same params otherwise:

```
PRECLEAR ENGAGEMENT: 4005 cells outside the boundary; planner reads
fully-cleared on 1745 with the boundary set vs 0 without it.
```

(1745, not 4005, because "fully cleared to the grid floor" only counts cells
with no mesh under them; covered cells are cleared to the *surface* Z, which is
still a clear but not to the floor.) The fixture is not vacuous.

### 1.3 The boundary-collapse escape hatch is NOT hit here

`effective_boundary_reported` returned **1 polygon, `offset_failure: None`**.
This fixture takes the normal clip path.

The escape hatch is nonetheless measured, because **the "UNclipped" arm below
is exactly what it ships**: on a genuine collapse,
`resolve_collapsed_containment` records the report-only
`boundary_clip_dropped` finding and the path is emitted with no clip at all.
Interesting inversion — the collapse arm has *less* planner/sim divergence
(2282 vs 5090 `sim_higher`) and zero in-scope over-claim, while being the
unbounded-over-cut case. Divergence is not the quantity that ranks these two.

---

## 2. Measured — the zone cross-tab

Zones, by cell centre:

* **inside effective boundary** — inside `containment ⊖ r`; cutter centres are
  allowed here, so cuts here survive the clip.
* **reachable band** — inside the containment but outside `containment ⊖ r`.
  Material here is reachable (a centre on the `⊖ r` ring sweeps out to the
  containment) but a centre here is not allowed.
* **OUTSIDE boundary** — outside the containment. The pre-clear's zone.
  Unreachable by a compliant tool.

### ContourParallel — boundary set, production clip applied

```
grid 7921 cells; boundary pre-clear zone 4005 cells (50.6%); moves 2267 (798 cutting)
| zone                          |  cells |  agree |  plan> |   sim> | max dz |  sim>% |
| inside effective boundary     |   2464 |   2098 |      9 |    357 |  9.464 |  14.5% |
| reachable band (b\eff)        |   1452 |    724 |      0 |    728 | 24.500 |  50.1% |
| OUTSIDE boundary (precleared) |   4005 |      0 |      0 |   4005 | 25.000 | 100.0% |
| whole grid                    |   7921 |   2822 |      9 |   5090 | 25.000 |  64.3% |
```

### AgentSearch — boundary set, production clip applied

```
grid 7921 cells; boundary pre-clear zone 4005 cells (50.6%); moves 980 (366 cutting)
| zone                          |  cells |  agree |  plan> |   sim> | max dz |  sim>% |
| inside effective boundary     |   2464 |   2053 |      7 |    404 |  9.508 |  16.4% |
| reachable band (b\eff)        |   1452 |    765 |     31 |    656 | 24.001 |  45.2% |
| OUTSIDE boundary (precleared) |   4005 |      0 |      0 |   4005 | 25.000 | 100.0% |
| whole grid                    |   7921 |   2818 |     38 |   5065 | 25.000 |  63.9% |
```

### The isolating arm — boundary set, clip NOT applied (ContourParallel)

```
grid 7921 cells; boundary pre-clear zone 4005 cells (50.6%); moves 2151 (1590 cutting)
| zone                          |  cells |  agree |  plan> |   sim> | max dz |  sim>% |
| inside effective boundary     |   2464 |   2461 |      3 |      0 |  1.747 |   0.0% |
| reachable band (b\eff)        |   1452 |   1451 |      1 |      0 |  0.651 |   0.0% |
| OUTSIDE boundary (precleared) |   4005 |   1723 |      0 |   2282 | 25.000 |  57.0% |
| whole grid                    |   7921 |   5635 |      4 |   2282 | 25.000 |  28.8% |
```

### Controls — `boundary: None`, everything else identical

| | divergent | `planner_higher` | `sim_higher` | max dz |
|---|---:|---:|---:|---:|
| ContourParallel | 5 | 5 | **0** | 1.747 mm |
| AgentSearch | 81 | 81 | **0** | 4.161 mm |

### What the three arms separate

| term | ContourParallel | AgentSearch | produced by |
|---|---:|---:|---|
| outside-containment over-claim, **pre-clear only** (unclipped arm) | 2282 | — | `path.rs:404-430` |
| outside-containment over-claim added by the **clip** un-cutting what the path did cut | 1723 | — | boundary-unaware waterline cuts + clip |
| **in-scope** over-claim (inside the containment), unclipped | **0** | — | — |
| **in-scope** over-claim (inside the containment), clipped | **1085** | **1060** | **the clip** |

The in-scope over-claim is **1085 / 3916 = 27.7 %** of the in-containment
population on ContourParallel and **1060 / 3916 = 27.1 %** on AgentSearch — at
0.28 mm² per cell, ≈ **304 mm² and 297 mm²** of standing, tool-reachable
material the planner's own bookkeeping records as removed.

---

## 3. Move-level probe — where the emitted centres actually go

Counted on the **pre-clip** toolpath, cutting moves only, endpoint XY against
the containment and its inset:

| | inside `⊖ r` | in the band | fully outside containment | max endpoint x |
|---|---:|---:|---:|---:|
| ContourParallel | 798 | **654** | **138** | 23.079 |
| AgentSearch | 366 | **100** | **138** | 23.079 |

* The clip converts **792 of ContourParallel's 1590 cutting moves (49.8 %)** and
  **238 of AgentSearch's 604 (39.4 %)** into rapids.
* The **138** fully-outside cutting endpoints are **identical between the two
  strategies** — same count, same radius range `15.047 .. 23.150 mm`, same Z
  set (clustered at z ≈ 1.00–1.55, the bottom of the ladder). 23.150 mm is the
  hemisphere's radius at z ≈ 1.0 (√(400−1) = 19.975) plus the tool radius
  (3.175) to three decimals. That identifies them as **`waterline_cleanup`
  contours** (`adaptive3d/clearing.rs:1200-1218`), which is
  strategy-independent, offsets mesh contours by the cutter radius, and
  **takes no `boundary` parameter at all**. It stamps the planner grid across
  the full mesh silhouette; the clip then rapids the outside portion.

---

## 4. Map artifacts — visual confirmation

`RS_CAM_PARITY_MAP=1`, legend: `.` agree, `p` planner_higher, `s` sim_higher
inside the effective boundary, `b` sim_higher in the reachable band, `S`
sim_higher outside the containment.

ContourParallel, clipped, rows 42–47 (mid-grid):

```
bbbbbbssss..................................SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
bbbbbbssss..................................SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
bbbbbbssss..................................SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
bbbbbbssss..................................SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
bbbbbbsssss.................................SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
bbbbbbsssss.................................SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
```

ContourParallel, **UNclipped**, the same rows — the standing wall is gone and
the outside-containment residue is an annulus (the waterline signature), not a
filled half:

```
.................................................SSSSSSSSSSSSSSSSSS...............SSSSSSS
.................................................SSSSSSSSSSSSSSSSSS...............SSSSSSS
```

ContourParallel, clipped, rows 1 / 85 (top and bottom containment edges) — the
same `b`/`s` wall runs along every containment edge, not only the x = 0 one:

```
bbbb....bbb.bbbbbbbb.....bbbbbbbbbbbbbbbbbbbSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
b.........................bbbbbbbbbbbbbbbbbbSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS
```

The wall is **~10 cells ≈ 5.3 mm ≈ 1.67 r** wide: ~6 cells in the band plus
~4 cells *inside* the effective boundary. `S` fills the entire right half.

**Artifacts** (ASCII map + PGM per arm, full run log, and the scratch
instrument's source, since the test itself was deleted from `crates/`):

```
/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/
  fe062f6e-ba96-4d15-8a99-f12da437e66d/scratchpad/boundary_lane/
    ContourParallel___boundary_half___clipped.map.txt / .pgm
    ContourParallel___boundary_half___UNclipped.map.txt / .pgm
    AgentSearch___boundary_half___clipped.map.txt / .pgm
    run_full.log
    research_boundary_parity_scratch_w7.rs.txt
```

---

## 5. Root cause

Two independent mechanisms, both live in production whenever a containment
boundary is set on adaptive3d.

### 5.1 The pre-clear polygon and the clip polygon differ by one tool radius

`session::compute::resolve_containment_polygon` returns the **un-inset**
containment and hands it to `Adaptive3dParams::boundary`
(`execute.rs:1576`). Its doc comment states the reasoning explicitly:

> "for adaptive3d's internal-stock pre-clip we want the silhouette itself,
> since the cutter footprint (when its center is at silhouette − tool_radius)
> reaches the silhouette boundary and validly stamps cells in that band."

That argument is correct **about the cells** and silent **about the moves**.
`apply_boundary_clip` gates cutter **centres** against
`effective_boundary(poly, Inside, r)` = `containment ⊖ r`. The argument
therefore only holds if the planner's emitted centres already respect
`containment ⊖ r`. Measured, they do not: **654 of 1590** ContourParallel
cutting-move endpoints (41 %) and **100 of 604** AgentSearch endpoints (17 %)
land in the band, and every one becomes a rapid.

The supported reading — labelled a hypothesis, since proving it means reading
the region/EDT code an implementation lane will read anyway — is that
`path.rs:404-430` sets **one** grid that is then used for two different
questions: *where is there material* (correctly bounded by the containment) and
*where may a cutter centre go* (must be bounded by `containment ⊖ r`). Only the
first bound is applied, so the clearing strategy places its outermost ring's
centres at the containment edge and the clip destroys them.

**Net effect:** a ~2 r-wide wall of in-scope, tool-reachable, uncut material
along every containment edge, that the planner's own dexel records as removed.

### 5.2 `waterline_cleanup` is boundary-unaware

`adaptive3d/clearing.rs:1200-1218` takes `mesh, index, cutter, lut, slope_map,
material_stock, z_level, tool_radius, cell_size, safe_z, tolerance,
min_cutting_radius, stock_to_leave, segments, last_pos, debug_ctx, cancel` —
**no boundary**. It traces mesh waterline contours offset by the cutter radius
and stamps them into `material_stock`, regardless of `params.boundary`. Those
segments are the 138 fully-outside cutting moves, identical on both strategies,
and they are exactly the O5b "cuts become rapids, the dexel is left unstamped"
mechanism the `params.boundary` pre-clear was written to eliminate — surviving
in the one emitter the pre-clear does not reach.

---

## 6. Consequence analysis — who consumes the planner claim grid?

**Nobody outside `adaptive3d`.** Traced, not assumed:

1. `Adaptive3dSegmentsResult::final_material_stock` — the planner's claim grid —
   appears at exactly **eight** sites in the whole workspace
   (`grep -rn final_material_stock crates/`): its declaration and doc in
   `adaptive3d/path.rs:136,149`, its construction at `path.rs:975`, three reads
   inside `debug_adaptive_3d_segments_for_f029_probe` (`path.rs:171,188,189`),
   and the two parity-test call sites in `adaptive3d/mod.rs:2437,2817`. The
   field carries `#[allow(dead_code)]` and the doc `/// Test-only`. It is never
   returned to `compute`, `session`, or the GUI.
2. **Rest machining runs the other way.** `StockSource::FromRemainingStock`
   reads `prior_stocks` (`compute/sim_prefix.rs:511`), which
   `compute::simulate` builds by replaying emitted toolpaths through the dexel
   **simulator**. That stock is then handed *into* the planner as
   `initial_stock: ctx.initial_stock.cloned()` (`execute.rs:1569`). The data
   flow is **sim → planner**, never planner → sim. A boundary-clipped rough
   therefore does **not** cause a rest op to skip the standing wall: the rest op
   sees the simulated stock, which carries it.
3. The `DerivedRestRegions` boundary source reads
   `result.annotated().rest_regions` from a pencil op's rest-depth detector —
   also sim-derived (`session/compute.rs:1722`).
4. The report-only generation findings that *sound* like they might carry a
   planner claim (`truncated_core_mm2`, `untouched_material_mm2`,
   `reached_uncut_estimate_mm2`) are written only by `record_truncated_core`
   (`execute.rs:175-188`), whose callers are the finishing-cascade reports
   (`ScallopReport` / `UnifiedFinishReport`). adaptive3d writes none of them, so
   they read `None` = **not measured** — correct, and not a silent zero.

**The one real consumer is intra-run:** each Z level's region detection reads
the same `material_stock`, so the planner plans nothing at deeper levels where
it believes it already cleared. That is what turns 5.1 from a bookkeeping
mismatch into standing material.

---

## 7. Severity verdict

| axis | rating | why |
|---|---|---|
| **Safety** | **LOW** | The divergence is one-sided in the *benign* direction for collisions: material is left standing, not removed extra. No deep bite was observed on this fixture (the standing wall is never subsequently cut). Collision detection and the sim's own metrics are unaffected — the sim is the arbiter and it sees the truth. |
| **Rest-machining correctness** | **NONE** | Refuted by the trace in §6: no rest generator reads the planner claim grid. |
| **Machining quality** | **MEDIUM** | Every adaptive3d rough with a containment boundary leaves a ~2 r wall of in-scope material along the boundary — 27 % of the in-containment cell population, ≈ 300 mm² here, up to 24.5 mm deep — and the planner does not know. Half the emitted cutting motion is paid for and thrown away as rapids (49.8 % of cutting moves on ContourParallel). |
| **Instrument blindness** | **MEDIUM-HIGH** | The shipped parity pair's PRIMARY bar reads "every `sim_higher` cell must lie inside the F-027 border-clear zone" and is worded as the bar that catches a mirror defect. In this configuration the border-clear zone is **0 cells** and `sim_higher` is **5090** — the bar would fail loudly *if any shipped test set a boundary*. None does. The blind spot is the whole boundary-enabled configuration. |

### On §7(a)'s prediction — confirmed, but incomplete

`DELTA_w5b_f1_planner_boundary.md` §7(a) predicted "the planner's internal
stock should over-claim outside the boundary in exactly the same direction, by
the same mechanism". Direction: **correct**. Mechanism: **correct but it is the
smaller term** — the pre-clear alone accounts for 2282 cells of unreachable
material with no consumer, while the clip accounts for 1085 cells of *in-scope*
material plus 1723 more outside. §7(a) did not predict the in-scope term at all,
because it did not model the tool-radius offset between the two polygons. Not a
refutation, so no new ledger entry; recorded so the next reader does not treat
§7(a) as a complete description.

---

## 8. Sentry proposal — sized for one implementation lane

**Warranted: yes.** The configuration is shipped, the divergence is large,
deterministic and reproducible to the cell, and no existing test exercises it.

Shape it as a **third arm** of the existing parity family, in-lib (so
`segments_to_toolpath` is reachable and the fixture stays one function):

**Instrument changes** — `crates/rs_cam_core/src/adaptive3d/mod.rs`, all inside
`#[cfg(test)] mod tests`:

1. Add `boundary: Option<Polygon2>` and `apply_clip: bool` to
   `run_planner_sim_parity_with_mesh`. When `apply_clip`, run the emitted
   toolpath through `boundary::effective_boundary(poly, ToolContainment::Inside,
   r)` + `clip_annotated_to_boundary_set` before simulating — mirror
   `session::compute::apply_boundary_clip`, and assert the two agree rather
   than assuming it (`clip_toolpath_to_boundary` is the `Toolpath`-level
   equivalent used by this lane's scratch instrument).
2. Extend `ParityResult` with `sim_higher_outside_containment`,
   `sim_higher_in_band`, `sim_higher_inside_effective`,
   `containment_population`, `band_population`, and
   `emitted_cut_moves_outside_containment`. Print them on the existing
   `eprintln!` line the way `W5B-F1:` already does.
3. New `assert_containment_parity_bars`.

**Bars** (measured 2026-08-21 on the fixture above; ContourParallel /
AgentSearch):

| # | bar | measured | proposed pin | rationale |
|---|---|---:|---|---|
| P1 | **precondition**, non-vacuity: pre-clear zone > 0 **and** `effective_boundary` returned ≥ 1 polygon | 4005, 1 | assert both | the empty-population trap, and it stops the fixture silently sliding onto the collapse escape hatch where the clip is a no-op |
| P2 | **precondition**: F-027 border-clear zone == 0 | 0 | `assert_eq!(…, 0)` | proves this arm measures the *boundary* clear and not W5B-F1's border clear |
| B1 | **in-scope over-claim** — `sim_higher` inside the containment, as a fraction of the in-containment population | 27.7 % / 27.1 % | **≤ 30.0 %** growth pin | the consequential term; a *percentage* here, not zero, because the defect is real and open |
| B2 | **waterline leak** — emitted cutting-move endpoints fully outside the containment | 138 / 138 | **`assert_eq!(…, 138)`** exact | deterministic and strategy-independent; a count bar is sensitive to one move where a percentage over 1590 moves is not. Change it only with a measurement |
| B3 | **outside-containment** `sim_higher` | 4005 / 4005 | **not gated** | those cells are unreachable by a compliant tool and have no consumer; gating them re-creates W5B-F1's mistake of pinning a deliberate pre-clear as if it were a defect. Excluded by construction from B1, exactly as W5B-F1's primary bar excludes the border-clear zone |
| B4 | `planner_higher` inside the containment | 9 / 38 | ≤ 60 | the benign direction; a loose growth pin only |

Plus **one `#[ignore]`d aspirational test** asserting B1 == 0, carrying the
fix contract in its doc so the day the pre-clear is aligned to
`containment ⊖ r` (and `waterline_cleanup` learns about the boundary) the test
is un-ignored rather than re-derived.

**Cost:** one lane, roughly a day. Two runs per strategy (~4.5 s each in a
debug test binary; the scratch instrument's three tests ran in 8.2 s total).
Verdict-safe by W5B-F1's own definition — every hunk inside `#[cfg(test)] mod
tests`, so no generated geometry can move.

**Fixes this would gate** (out of scope for this lane, listed so the sentry's
bars have a destination):

* **F-A** — stop conflating "where material is" with "where a centre may go".
  Either pre-clear on `containment ⊖ r` (one line; re-opens O5b for the band,
  so probably wrong on its own) or bound the clearing strategy's centre
  placement by `containment ⊖ r` while keeping the material grid at the
  containment (correct, larger).
* **F-B** — pass `params.boundary` into `waterline_cleanup` and drop contour
  points outside it. Small and self-contained; would take B2 from 138 to 0.

---

## 9. Reproduction

The scratch instrument was `crates/rs_cam_core/tests/research_boundary_parity_scratch_w7.rs`,
**deleted** per the lane's read-only constraint; its source is preserved at
`…/scratchpad/boundary_lane/research_boundary_parity_scratch_w7.rs.txt`.
Restore it and run:

```sh
RS_CAM_PARITY_MAP=1 cargo test -p rs_cam_core \
  --test research_boundary_parity_scratch_w7 -- --nocapture --test-threads=1
```

Every figure in this document comes from that one binary; the full log is at
`…/scratchpad/boundary_lane/run_full.log`.
