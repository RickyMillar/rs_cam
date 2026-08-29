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
