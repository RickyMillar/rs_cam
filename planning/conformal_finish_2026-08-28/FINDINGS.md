# Conformal finishing — findings

## §F1-1 Direction-field arm on captured Wanaka region 1 — FALSIFIED (2026-08-30)

Instrument: `crates/rs_cam_core/tests/direction_field_wanaka_f1.rs`
(`wanaka_direction_field_f1`, run against capture `0206aa69`, module at
`f1e1447e`). Full stage output in the run log; SVG artifact
`wanaka_region1_direction_field_f1.svg` (drop in
`~/Downloads/svg/conformal_f1/`).

### Stage F1-A — solve + cheap falsifier

| quantity | value |
|---|---|
| region triangles / vertices | 43,783 / 23,641 (1 component) |
| BFS seeds / singular tris / transported | 3 / 0 / 0 |
| **orientation inconsistencies** | **2,340** (5.3% of triangles) |
| clamped \|V\| triangles | 67 |
| \|V\| min / mean / max | 0.0070 / 0.3229 / 0.4613 |
| CG iterations / rel. residual | 2,682 / 9.9e-11 (converged) |
| levels | **253** (≈51 expected from √h at target magnitude) |
| components per level min/med/max | 1 / **28** / 57 |
| closed loops / saddles / floored increments | 286 / 0 / 11 |
| **TOTAL polylines** | **6,589** |

**Falsifier verdict: FAIL / STOP.** 6,589 polylines vs the 141-fragment
PCA-cell reference (46.7×) and the 564-fragment 0° undivided raster (11.7×).
Both references are tapered-ball counts, but the falsifier compares
fragmentation, not cutter-matched cost — and the margin is not close.
Stages C–E (spacing, CL/cost, baselines) were skipped by design; no time
was measured and none is claimable.

### Diagnosis (why, not just that)

Three compounding causes, all visible in the numbers and the SVG:

1. **Structural — a global scalar cannot serve a branched ribbon.** Region 1
   is a long, looping, branching ribbon (the "thin organic" name is
   literal). Every iso-level of a single φ must thread ALL arms it touches
   at once: median 28 components per level × 253 levels ≈ the 6,589 total.
   This is not a tuning artifact; it is what level sets of one global
   function on this topology look like. The paper's test surfaces (blade,
   bike seat, saddle) are all simply-connected convex-ish sheets.
2. **The curvature-derived D field is noise on a Shallow region.** The
   region is the *Shallow* band by construction — near-umbilic almost
   everywhere — so t₁ (max signed principal direction) is barely determined:
   2,340 orientation inconsistencies (5.3%), visible as fingerprint-like
   swirl patches. The paper's own singular-region machinery never engaged
   (0 triangles flagged) because the anisotropy is small-but-nonzero rather
   than zero: the noise is confidently wrong, not detectably absent. A
   preferred-direction field derived from curvature has no signal exactly
   where the finishing problem lives (shallow terrain).
3. **The conservative min-increment rule over-densifies.** 253 levels
   against ~51 expected: the global-minimum increment (paper §3.3) is set
   each level by the worst crossing, and with a noisy gradient the worst
   crossing is always bad. 5× the passes = 5× the cutting distance before
   any linking argument starts — the §0k path-length trap (+43% killed
   contour rings; this is +400% before clipping).

### Cross-evidence

Independently consistent with the thin-organic campaign's same-day
refutations (`planning/thin_organic_2026-08-27/FINDINGS.md` §0j, §0k):
per-cell direction variation measured 0.917× (a cost), contour-per-cell
0.686×, and D3's bound that any per-cell mix of shipped patterns is
break-even at best. Four independent instruments now agree: **on
region-1-class shallow organic geometry, direction variation does not pay;
the PCA-frame monotone-cell raster (C2, production-validated 1.090×) is the
right answer there.**

### Verdict and scope

- Arm A (direction-field iso-level paths) is **dead on the Wanaka
  region-1 class**: shallow, branched, multiply-connected organic regions.
  The charter's F1 advance bar cannot be met there.
- What survives of arm A: nothing for this fixture. If it ever returns, it
  is for **steep, curved, simply-connected** surfaces where curvature
  actually determines a direction (the paper's own test class) — no such
  surface is currently a priority fixture, so this is recorded, not
  planned.
- The research module (`direction_field.rs`) stays: it is the working
  Poisson/marching-triangles substrate, its unit tests pin the numerics,
  and F2's evaluation machinery reuses parts of it (curvature access,
  region submesh, iso-curve extraction).

### What this does NOT falsify

The conformal-spiral arm (F2) is a different mechanism — stay-down
concentric rings from a *parameterisation*, not level sets of a fitted
scalar; hole avoidance is by construction (arc slits), not by threading.
The F2 reading gate is OPEN (`reading_set_gate_status.md`). The §0k
path-length trap and the 755.3 s ceiling-arm bar (thin-organic §0i) are the
hazards F2 must answer, with `relink_and_cost_under` as the mandatory
kernel for its ceiling arm.

## §F1-2 Segmentation works. It does not fix the fragmentation. (2026-08-31)

Instrument: `crates/rs_cam_core/tests/direction_field_wanaka_f1.rs`,
test `wanaka_direction_field_segmented_f1`, commit `b2d2b346`.

§11 of the synthesis reopened this arm. The literature runs four stages
after the direction rule. We had built none of them. Segmentation was the
stage that should stop the level sets of one global scalar from crossing
every branch of region 1. This test adds it.

### The mechanism check passes

Components per level, measured:

| arm | patches | levels | polylines | components/level |
|---|---|---|---|---|
| 180° undivided control | 3 | 197 | 13,489 | **68.47** |
| 45° coherence patches | 1,452 | 4,752 | 9,536 | **2.01** |
| 30° coherence patches | 2,925 | 7,875 | 10,514 | **1.34** |
| 20° coherence patches | 5,146 | 10,931 | 12,867 | **1.18** |
| 10° coherence patches | 11,380 | 17,353 | 18,562 | **1.07** |

Segmentation does what the literature says it does. Threading falls from
68.47 components per level to 1.07. Orientation inconsistencies fall from
3,915 to zero. The stage is not broken.

### The verdict fails anyway

The pre-registered bar asked for an order of magnitude fewer polylines. The
best arm gives control ÷ 1.41. Tighter tolerances make the count worse, not
better. The second gate criterion, which lets a patch bend, reaches 9,119
polylines at 30°. That is the same order.

**Pre-registered reading: segmentation is not the fix.**

### Why, and this is the new finding

**Anisotropy magnitude is not direction coherence. The method needs both.**

§11 measured region 1's anisotropy and found it real: median `W_max/W_min`
1.0950 at R = 1.0 mm, a +9.75 % prize ceiling, confirmed as landscape by
both scale rules. That measurement says the surface *has* a preferred
direction at each point. It does not say that direction *holds over an
area*.

Region 1's direction field turns continuously. Any coherence-based
segmentation therefore cuts the region wherever the field has turned past
the tolerance. The patch count then rises as fast as the threading falls,
and the two effects cancel. The sliver census shows the cost directly: at
45° the arm makes 1,064 patches under 1 mm² (5.4 % of the region); at 10°
it makes 10,640 of them, and **48.2 % of the region lies in patches smaller
than 1 mm²**. The segmentation shreds the surface.

The paper's surfaces do not behave this way. A turbine blade or a bike seat
carries a few large coherent zones. River-carved terrain does not.

### Status of the arm

Falsified on region 1, now for a understood reason rather than a suspected
one. The chain of three claims is complete:

1. The premise holds — the anisotropy is real and the prize is percent-scale
   (§11).
2. The missing stage works — segmentation removes the threading (this
   section).
3. The method still fails — because the field lacks the spatial coherence
   the method needs, and no stage in the literature's pipeline supplies it.

Reopening the arm again requires a surface with large coherent direction
zones. Wanaka is not one. That is a property of the geometry, measurable in
advance from the sliver census at a chosen tolerance, and cheap to check
before any future attempt.
