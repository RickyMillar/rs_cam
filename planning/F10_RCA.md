# F.10 RCA — Arc-fit producing implausibly large arcs on adaptive3d

## Repro

Fixture: `/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml`
Toolpath: index 6, name "3D Rough 6" (3D Rough / adaptive3d).
(The pain-points doc calls this TP10; in the loaded fixture it is 0-based
index 6 — "3D Rough 6". 1782 moves, 4439mm cutting, 3136mm rapid.)

Live MCP narration on master @ `670cdb2`:

```
⚠ 4 perimeter sweep arc(s) with R > tool_radius × 30 (smallest 91.5mm,
  largest 257.3mm). First: move 500, CW, z=22.000, center=(-34.2, -57.1),
  target=(75.5, 36.2).
```

Tool 3 is a 6mm end-mill (radius 3mm); the trip threshold is therefore
`3.0 * 30 = 90.0 mm`. Four arcs in this generation exceed it; the
largest is 85.8× tool radius. The pain-points doc records "5 ... 91.5mm
... 257.3mm" — same generation behaviour, the exact count varies by one
between runs because of an off-by-one near a near-90mm boundary.

The flag fires at both stepover=2.0 and stepover=2.6 (per the
pain-points doc), so it is a generation behaviour independent of the
optimizer. No gate trips on it — only the narration surfaces it.

## Where the arcs come from

**Pipeline trace:**

1. `crates/rs_cam_core/src/compute/execute.rs:1375-1393` — the dressup
   stage runs `crate::arcfit::fit_arcs(at, tolerance)` whenever
   `cfg.arc_fitting` is `true`.
2. `crates/rs_cam_core/src/compute/config.rs:340` — `DressupConfig.arc_tolerance`,
   default `0.05` (line 367).
3. `crates/rs_cam_core/src/compute/config.rs:388-394` — for the
   `Roughing` `UiProcessRole`, `for_role` overrides the global
   `arc_fitting: false` default to `arc_fitting: true`. adaptive3d is
   roughing, so fresh toolpaths get arc-fit enabled.
4. The wanaka project file confirms this: `wanaka_full_tuned.toml` line
   694 has `arc_fitting = true`, line 695 has `arc_tolerance = 0.05` in
   the dressups block for "3D Rough 6".
5. Inside `fit_arcs` (`crates/rs_cam_core/src/arcfit.rs:26-222`), runs
   of consecutive linear moves at constant Z and feedrate are
   greedily extended while `try_fit_arc` keeps succeeding, then
   collapsed into a single `ArcCW`/`ArcCCW` move that carries
   `i, j` offsets from start to centre.
6. After arc-fit, narration scans every arc move via
   `arc_observations` (`crates/rs_cam_core/src/narrate.rs:860-892`),
   computing radius as `sqrt(i*i + j*j)`. Anything above
   `tool_radius * LARGE_ARC_RADIUS_MULTIPLIER` (constant `30.0` at
   `crates/rs_cam_core/src/narrate.rs:20`) is appended as an anomaly
   (`crates/rs_cam_core/src/narrate.rs:820-858`).

`inspect_spans(index=6, kind="dressup_artifact")` on the loaded
fixture reports 93 `DressupArtifact / "arc-fit"` spans. The warning
fires on the subset whose `|IJ|` exceeds 90mm. Notable wide-source
spans (start..end in the *new* toolpath, but each arc collapses an
N-move source run — see below):

| arc span | new-index width | collapsed source segments (approx) |
|----------|-----------------|------------------------------------|
| 1208..1210 | 2 | (multi-move arc — 2-move output is rare) |
| 1291..1293 | 2 | likewise |
| 1658..1660 | 2 | likewise |

Most arc-fit spans are width 1 (single arc move that replaced a multi-
segment source run). The source-side run length is not directly
recorded on the span; it's reconstructed only via the `MoveRemap`
collapse (line 173-178 of `arcfit.rs`).

## Arc-fit implementation

`fit_arcs` at `crates/rs_cam_core/src/arcfit.rs:26`:

- Walks the toolpath looking for a starting `Linear` move.
- Greedily extends `end_idx` over consecutive `Linear` moves with the
  same feedrate and `|dz| <= tolerance` (lines 86-99).
- Truncates at `RapidOrderBarrier` / `DepthPass` boundaries (101-110).
- Calls `try_fit_arc` on `start + [i..run_end]` for progressively
  longer `run_end` and keeps the **longest** run that fits
  (`while run_end <= end_idx`, lines 128-141). On the first failure
  it stops extending and emits the longest accepted arc.

`try_fit_arc` at `crates/rs_cam_core/src/arcfit.rs:233`:

- If `>=5` points: least-squares circle fit via Kåsa's algebraic
  method (`circle_from_least_squares`, lines 312-370).
- Otherwise: 3-point circumscribing circle through first / middle /
  last point (`circle_from_3_points`, lines 373-401).
- Rejects only if `radius > 1e6` (line 250-252) — a 257mm radius is
  six orders of magnitude under that cap.
- **Per-point radial check** (lines 254-262): every point must be
  within `tolerance` (0.05mm) of the fitted circle. For radius
  R=257mm this is easy — a 0.05mm radial tolerance over 257mm is
  ~2×10⁻⁴ fractional, which a barely-curving polyline can satisfy
  by accident.
- **Per-chord sagitta check** (lines 274-290): for each
  consecutive pair, computes
  `sagitta = R − sqrt(R² − (chord/2)²)` and rejects if `sagitta > tol`.
  This is the rectangle-corner guard added in commit `b312b75` (the
  regression test at line 477-492 catches a 100mm-chord 70mm-R fit
  with 21mm sagitta).
- **Missing**: there is no check on the absolute or
  `R / tool_radius` magnitude of the fitted radius, and no check on
  the total swept angle of the resulting arc.

`fit_arcs` (the outer loop) does **not** distinguish "small R, many
segments fitted" from "huge R, many segments fitted" — both pass the
greedy extension as long as each next segment keeps the per-point
and per-chord checks happy.

## What the wanaka arcs actually look like

The first flagged arc per the live narration is move 500, CW,
z=22.000, with centre `(-34.2, -57.1)` and target `(75.5, 36.2)`.

Pass z=22.000 is the first depth pass on the back face. The narration
reports for that Z-level:
- `82 arcs` at z=22.000 pass 1 (1021 cutting moves total)
- perimeter sweep estimate 72.5mm radius from centroid
- 3509mm cutting at this Z

So the **typical** geometric scale at this Z is ~72.5mm radius — the
**fitted** arc R of 257mm is ~3.5× larger than the natural perimeter-
walk scale. That is the smoking gun: the fitted circle's centre is
displaced well outside the perimeter ring, and its radius is much
larger than any real geometric feature of the toolpath at this depth.

### Sagitta-tolerance arithmetic

With `tolerance = 0.05mm`, a single chord can stretch to
`chord_max ≈ 2 * sqrt(2 * R * tol)` before the sagitta check trips.

| R    | max single-chord length | per-segment sweep θ |
|------|-------------------------|---------------------|
| 30mm | 1.7mm                   | 3.3°                |
| 90mm | 3.0mm                   | 1.9°                |
| 257mm| 5.1mm                   | 1.1°                |

So at R=257mm, the algorithm will accept ~5mm-long chords as
"on-arc". A run of 10–20 such segments traces a few centimetres of
gently curving path (sweep angle 10°-20°) — but the algebraic best-
fit circle for that path can place its centre 250mm away from the
toolpath if the polyline is nearly straight with the tiniest
consistent bow.

### Why this happens with the least-squares fit specifically

Kåsa's algebraic method minimises `Σ(x² + y² + Dx + Ey + F)²`. It is
known to be **biased** toward large circles when points are noisy or
span a small fraction of the circumference. With sub-millimetre point
spacing and a path that's *almost* straight, the residuals in the
algebraic-distance space are minimised by sliding the centre far
away — geometric distance to the circle stays under 0.05mm, but the
solution is essentially "this polyline is a chord of an enormous
circle".

This matches the symptom: small per-point residuals (fit "succeeds"),
huge R, sweep angle that's tiny in radians. The 257mm-R arc on a
~72mm perimeter is the algorithm reporting "your wiggly perimeter
segment happens to look like a chord of a 257mm circle to within
0.05mm".

## Risk assessment

There are two separable risks:

### 1. G-code validity (controller risk)

The emitter at `crates/rs_cam_core/src/gcode/emitter.rs` and the
post-processors at `crates/rs_cam_core/src/gcode/post.rs` emit arcs
using **IJK incremental centre** form — `G2 X Y I J F` — not radius
form (R-word). IJK form is unambiguous regardless of R: the controller
just walks from current position to (X, Y) along the arc whose
centre is current + (I, J).

`should_linearize_arc` (`emitter.rs:80-86`) linearises only when
`r < threshold_mm` (0.05mm default). There is no **upper-bound**
linearisation. All shipped posts (grbl, grblHAL, linuxcnc, mach3)
enable arc-linearisation at the 0.05mm sub-mm threshold (tested at
`gcode/post.rs:451-464`).

So a 257mm-R G2/G3 will be emitted as-is. **Likely controller
behaviour:**

- **LinuxCNC, Mach3, modern grblHAL**: accept IJK arcs of any
  geometrically-valid radius. They interpolate the actual arc.
- **Older Grbl 1.1**: also accepts IJK arcs and interpolates
  internally; no known issue with large R.
- **Offline parsers / preview tools that synthesise R-form**: some
  third-party preview tools convert IJK → R for display and choke
  on R values that don't match the start/end-distance arithmetic.
  We don't ship one, so this isn't a direct risk surface.

**Bottom line**: the IJK arcs emitted should run correctly on
mainstream router controllers. The chord-error guarantee (≤0.05mm
sagitta vs the original cut path) **does** still hold — the polyline
is genuinely close to the fitted arc, by construction.

### 2. Cut-path fidelity (the more interesting question)

The chord-vs-arc sagitta check **does** bound the maximum deviation
of the new arc from the original polyline at 0.05mm. So the **cut
path** doesn't shift by more than 0.05mm — and 0.05mm is well below
the resolution of any adaptive3d generator output.

What's lost is just **semantic legibility**: the G-code reads
"arc of radius 257mm" instead of "10 short segments along a barely-
curving line". An operator inspecting the output won't see a 257mm
feature anywhere on the part, which is alarming if you're trying to
correlate G-code with geometry. That's the user-experience problem
F.10 is trying to nail.

### Combined assessment

- **Safety / "will it crash"**: low risk. Chord error is bounded.
  IJK form is unambiguous on all our supported controllers.
- **Surprise / interpretability**: high. The narration ⚠ is correct
  to flag this — large-R arcs in adaptive3d output indicate the
  arc-fit is being **opportunistic** rather than recovering true
  geometry. Operators reasonably expect arc moves to map to circular
  features in the model.
- **Mode failure case**: if a future refactor swaps IJK → R-form
  emission (e.g. for a controller that prefers R), the 257mm arcs
  become a real problem — the R-word ambiguity is huge for arcs
  that subtend < 180° on huge circles. We're one post-format change
  away from a real bug, so the safety net here is thin.

## Suggested fix paths

Ranked by simplicity × impact:

### A. Sanity cap on fitted radius — cheap, high signal

In `try_fit_arc` (`arcfit.rs:233`), reject fits where
`radius > some_cap`. Options for the cap:
- Absolute: `radius > 1000mm` (anything router-scale should fit
  inside a ~1m envelope arc).
- Relative: `radius > 50.0 * tool_radius` or similar. Tool radius
  is known to the dressup pipeline via `OperationContext`.
- Relative-to-toolpath-bbox: precompute the toolpath's XY bbox
  diagonal; reject `radius > 2.0 * bbox_diagonal`. Most defensible
  but requires plumbing the bbox into `fit_arcs`.

This eliminates the algebraic-fit-runaway case entirely while
preserving all "real" arcs in the model (typical part features are
2–50mm radius for the wanaka tool envelope).

**Recommended starting point**: relative cap on `R / tool_radius`,
mirroring the narration's existing `LARGE_ARC_RADIUS_MULTIPLIER = 30`.
If the narration calls R > 30× a problem, the fitter shouldn't
emit it. Plumb tool radius into `fit_arcs` (it already lives on
the `OperationContext` consumed by `execute.rs`).

### B. Bound the swept angle — moderate, also high signal

Add to `try_fit_arc` a check on the total swept angle of the fit:
- Compute angles of first and last point about the fitted centre.
- Reject if the resulting sweep angle is below some threshold
  (e.g. < 5°). A 5°-sweep arc at huge R is the canonical "barely
  curving polyline misidentified as an arc" failure mode.

Composes with (A): a small-sweep, large-R fit is the precise bad
case; either gate stops it.

### C. Tighten `arc_tolerance` default — narrowest, lowest-cost

Drop `arc_tolerance` default from 0.05mm to e.g. 0.01mm. Reduces
the maximum chord length the fit will accept, which proportionally
reduces how far the algebraic-fit centre can wander before some
point pops outside tolerance.

Downside: legitimate arc-fits on tight curves emit more / smaller
arcs (or get rejected and stay linear), partially undoing the
G-code-size benefit of arc-fit. Doesn't fully solve the problem —
the wanaka 257mm-R cases still pass at 0.01mm if the polyline is
sufficiently bow-shaped (sagitta is the cube-root-ish of R*tol).

### Other options considered, not recommended

- **Drop least-squares, fall back to 3-point everywhere**: would
  also avoid the algebraic-bias-to-huge-R failure, at the cost of
  removing the noise-robustness benefit that least-squares gives
  on real arcs with sub-millimetre tessellation.
- **Disable arc-fit for adaptive3d**: removes the symptom; throws
  out the G-code-size win that arc-fit was added for in the first
  place. Adaptive3d emits a lot of moves and benefits from arc-fit
  on real curved features.

## Recommended path

**Option A** (relative `R / tool_radius` cap inside `try_fit_arc`)
with the same `30.0` constant the narration already uses. One-line
change to the threshold, ~10 lines to plumb tool radius through.
Add a regression test mirroring the wanaka case: a barely-curving
polyline (e.g. 20 points along `y = 0.001*x²` from x=0 to x=20)
should not fit a > 90mm arc with tool_radius=3.

Optionally combine with **Option B** (5° minimum sweep) for
defence-in-depth — the two checks are cheap and orthogonal.

## File:line references (verified against HEAD @ 670cdb2)

- `crates/rs_cam_core/src/narrate.rs:20` — `LARGE_ARC_RADIUS_MULTIPLIER = 30.0`
- `crates/rs_cam_core/src/narrate.rs:820-858` — `append_large_arc_anomalies`
- `crates/rs_cam_core/src/narrate.rs:860-892` — `arc_observations` (radius = `sqrt(i²+j²)`)
- `crates/rs_cam_core/src/arcfit.rs:26-222` — `fit_arcs` (greedy extension)
- `crates/rs_cam_core/src/arcfit.rs:233-306` — `try_fit_arc` (the inner checker)
- `crates/rs_cam_core/src/arcfit.rs:250-252` — `radius > 1e6` early-reject
- `crates/rs_cam_core/src/arcfit.rs:254-262` — per-point radial tolerance
- `crates/rs_cam_core/src/arcfit.rs:274-290` — per-chord sagitta tolerance
- `crates/rs_cam_core/src/arcfit.rs:312-370` — `circle_from_least_squares` (Kåsa)
- `crates/rs_cam_core/src/arcfit.rs:373-401` — `circle_from_3_points`
- `crates/rs_cam_core/src/arcfit.rs:477-492` — `test_fit_arc_rejects_rectangle_corners` (existing regression)
- `crates/rs_cam_core/src/compute/config.rs:340,367` — `arc_tolerance` field + 0.05 default
- `crates/rs_cam_core/src/compute/config.rs:388-407` — `for_role` enables arc_fitting for all three roles
- `crates/rs_cam_core/src/compute/execute.rs:1375-1393` — arc-fit dressup stage
- `crates/rs_cam_core/src/gcode/emitter.rs:77-86` — `should_linearize_arc` (only sub-mm)
- `crates/rs_cam_core/src/gcode/post.rs:120-147` — `ArcLinearize` (threshold_mm = 0.05 default)
- `crates/rs_cam_core/src/gcode/post.rs:451-464` — test: all shipped posts enable arc_linearize
- `crates/rs_cam_core/src/gcode/program_builder.rs:53-75, 342-365` — `ArcCW`/`ArcCCW` → IJK emission
- Fixture: `/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml:650-695` — TP "3D Rough 6" dressup block with `arc_fitting=true, arc_tolerance=0.05`
