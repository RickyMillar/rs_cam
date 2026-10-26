# fix24 (C43): the depth-weighted load oracle

Operator ruling, 2026-10-27: the load bound weights the engagement by the
axial depth. The load is the chip area, radial width × axial depth, against
the area of the bound at a full pass. A thin bottom layer across the disc
is light. A deep side cut is still held.

C18 owns the measurement side: the exact oracle, the formula of the C12
heavy test judge, and the fixture counts. C22 owns the planner side
(`P3_C22_3d_turns.md` §18). Python only. No cargo, no commit, no code
edit in the crates.

## 1. The formula (one formula for the planner and the judge)

Symbols: R the tool radius, D = 2R, d the op's Depth/Pass, z the tool tip
Z, H(x, y) the stock top before the step, u the lateral coordinate
(perpendicular to the XY travel of the step).

1. A disc point is uncut when H(x, y) − z > 0.01 mm (the 0.01 of the C12
   oracle, "at or below z + 0.01").
2. h(u) = the largest H − z of the uncut points of the disc at lateral
   offset u. The disc is the one at the step midpoint, as in
   `exact_width_at`. h(u) is the axial depth that the side of the cutter
   meets at u.
3. The chip area A = ∫ h(u) du. On the lattice: A = Σ over the lateral
   bins of h(u) · pitch, one bin per lattice pitch.
4. The load L = A / (D · d), a fraction like the radial fraction.
5. The bound is the number that the radial bound uses now:
   L ≤ ceiling + (planner cell + tolerance + exact pitch) / D. That is
   0.471 at s 2, R 3. The sim screen is ceiling + (planner cell + sim cell
   + tolerance) / D.
6. The sim screen in box form: L_box = radial_woc_fraction ×
   axial_engagement_mm / d. A sample is read exactly when the sim radial or
   L_box is over the screen.

Status: sent to C22 on 2026-10-27 for agreement (see §8).

## 2. The derivation (no new number)

- At a full pass the stock stands d above the tip over the whole engaged
  band, so h(u) = d there. Then A = (w·D)·d and L = w. The area load is
  equal to the radial fraction at a full pass. So the ceiling
  (1 − cos(1.05 · acos(1 − s/R))) / 2 and its tolerance terms carry over
  with no change: `radial_woc_fraction_from_leading_arc(pass_engagement_limit(s, R))`,
  `PASS_ENGAGEMENT_TOLERANCE` 0.05.
- A thin layer of height t < d across the band reads L = w·t/d: light.
- A cut deeper than d reads L > w: held, and held tighter than now.
- The lattice reads A to one bin: |ΔL| ≤ pitch · h / (D · d). Where
  h ≤ d that is pitch / D = 0.0083 at pitch 0.05, R 3, the exact-pitch
  term that the bound already holds.
- d is the Depth/Pass of the op that is read, from its project file:
  Corne op 1 "3D Rough 1" 6.0 mm (`corne_job_corrected_c12_agent_search.toml`
  line 127), wanaka200 op 1 "2 Back Rough" 5.46 mm
  (`wanaka200_mt1_c12_rough_only.toml` line 256). The C12 test uses
  `DEPTH_PER_PASS` 3.0, and `TOP_Z − FLOOR_Z` where a test sets that as
  the Depth/Pass. The judge must read d from the op's own parameters.

## 3. The oracle: `area_oracle.py`

- The height field H is a lattice of pitch 0.05 mm (the C12
  `EXACT_PITCH_MM`, sim cell / 10) over the stock box. H starts at the
  stock top. A point outside the box holds no stock. wanaka200: the
  samples are in the setup frame of the flipped stock, so the box is
  x 0–240, y 0–250, z 0–25 (`fix9/stockmap.py`).
- The samples are walked in trace order. Each sample is read first, then
  its step (the previous sample of the same toolpath to it) is stamped: a
  flat end mill of R 3 mm sets H = min(H, z) in its disc, at points 0.3 mm
  apart or less along the step, with Z linear along the step. So a read
  sees every earlier step in full, and the current move up to the previous
  sample (`from`), as `exact_width_at` does. The longest step is a helix
  chord: 0.52 mm on Corne, 1.80 mm on wanaka200.
- The read: the disc of R at the step midpoint, the tip Z at the step
  midpoint, u along the left normal of the XY step. It gives A, L, the
  exact width w (the lateral extent of the uncut points, over D) and hmax.
- Usage: `area_oracle.py <run> <corne|wanaka200> [toolpath id] [Depth/Pass]`.
  A later toolpath (for example a CP rest op) is read on the stock that the
  earlier toolpaths of the trace leave. Every toolpath stamps with R 3 mm,
  so a toolpath with another tool before the read toolpath gives a wrong
  stock (the Corne copy op is a 3 mm tool, toolpath 3).

The synthetic checks (`synthetic.py`): a straight pass, then a parallel
pass at stepover e on stock of height t over the tip (d = 6.0 mm, R 3 mm).
The second pass is read away from its ends.

| stepover e (mm) | stock height t (mm) | heading (rad) | L read | L = e·t/(D·d) | w read | w = e/D |
|---|---|---|---|---|---|---|
| 2.0 | 6.0 | 0.0 | 0.3333 | 0.3333 | 0.3333 | 0.3333 |
| 1.2 | 6.0 | 0.0 | 0.1917 | 0.2000 | 0.1917 | 0.2000 |
| 2.0 | 0.5 | 0.0 | 0.0278 | 0.0278 | 0.3333 | 0.3333 |
| 1.2 | 3.0 | 0.0 | 0.0958 | 0.1000 | 0.1917 | 0.2000 |
| 2.0 | 6.0 | 0.5 | 0.3288 | 0.3333 | 0.3288 | 0.3333 |
| 1.2 | 1.0 | 0.5 | 0.0326 | 0.0333 | 0.1958 | 0.2000 |
| 2.0 | 9.0 | 0.5 | 0.4932 | 0.5000 | 0.3288 | 0.3333 |

Each L is within one bin, pitch · t / (D · d), of e·t/(D·d), and each w
is within pitch / D = 0.0083 of e/D. At t = 9.0 mm > d the load is
0.4932 against w 0.3288: a deep side cut is held.

## 4. The C12 heavy test judge (proposal; not applied)

The test is `crates/rs_cam_core/tests/adaptive3d_geodesic_arm_c12.rs` on
the `adaptive-instruments` branch. The change that carries §1:

```rust
/// C43: the depth-weighted load of the sub-segment `from → to` of move
/// `m` (`phase3/fix24/README.md` §1). The lattice of [`exact_width_at`]
/// with a height per point: `TOP_Z`, lowered to the tip Z of every earlier
/// move where its disc passes (Z linear along the move), then of move `m`
/// up to `from`. h(u) is the largest uncut height (more than 0.01 mm over
/// the tip) at lateral offset u in the disc at the midpoint;
/// A = Σ h(u) · pitch and the load is A / (D · depth_per_pass).
fn exact_area_load_at(
    tp: &Toolpath,
    m: usize,
    from: (f64, f64),
    to: (f64, f64),
    depth_per_pass: f64,
) -> f64 {
    let r = TOOL_RADIUS;
    let pitch = EXACT_PITCH_MM;
    let half = 2.0 * r;
    let n = (2.0 * half / pitch) as usize + 1;
    let (x0, y0) = (to.0 - half, to.1 - half);
    let mut top = vec![TOP_Z; n * n];
    let stamp = |top: &mut Vec<f64>, px: f64, py: f64, pz: f64| {
        // the loop of `exact_width_at`'s stamp, with
        // `top[j * n + i] = top[j * n + i].min(pz)` in place of `cut[..] = true`
    };
    // every earlier move in full, then move `m` up to `from`, with
    // `move_points` extended to give the Z of each point (linear along the move);
    // `tip` is the mean of the Z at `from` and at `to`
    // ...
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l = dx.hypot(dy).max(1e-9);
    let (sx, sy) = (-dy / l, dx / l);
    let mid = (0.5 * (from.0 + to.0), 0.5 * (from.1 + to.1));
    let mut h = vec![0.0_f64; (2.0 * r / pitch).round() as usize + 2];
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (x0 + i as f64 * pitch, y0 + j as f64 * pitch);
            let above = top[j * n + i] - tip;
            if above > 0.01 && (x - mid.0).hypot(y - mid.1) <= r {
                let u = (x - mid.0) * sx + (y - mid.1) * sy;
                let b = ((u + r) / pitch).round() as usize;
                h[b] = h[b].max(above);
            }
        }
    }
    h.iter().sum::<f64>() * pitch / (2.0 * r * depth_per_pass)
}
```

In `read`: a judged sample is screened when `radial > screen` or
`radial * s.axial_engagement_mm / depth_per_pass > screen`. It fails when
the move's peak of `exact_area_load_at` is over `exact_bound()`. The
`screened:` lines print both peaks, the width and the area load.

## 5. The fixture counts

The old count (C22 §16.9; `fix21`–`fix23` `load_split.py`) is the XY length
of op 1's clearing_cut samples with sim radial > 0.471. The new count is
`area_over_mm`: the XY length of the read samples with L > 0.471
(`rescore.py`). The old count is kept beside it. The old counts here are
equal to `P15_D_verify.md` (D26, D33 default, D33 stock_depth) and to
`P3_C38_outer_band.md` §16.4 (C35).

## 6. The re-score (read only)

Classes of the old count, in this order: sim-only (L ≤ 0.471 and the exact
width w ≤ 0.471), thin layer (w > 0.471 and L ≤ 0.471), still over
(L > 0.471). "Lowered feed": a sample feed below the op's base clearing
feed, the most common one (`load_split.py` rule; the C38 fed slots and the
capped turns).

### Corne, op 1 "3D Rough 1"

| | D26 (`fix16/corne_pick`) | C35 (`fix12/corne`) | D33 (`fix23/default/corne`) | D33 stock_depth (`fix23/stock_depth/corne`) |
|---|---|---|---|---|
| op 1 Depth/Pass d (mm) | 6.00 | 6.00 | 6.00 | 6.00 |
| old count: sim radial > 0.471 (mm) | 780.8 | 486.3 | 1044.1 | 1058.8 |
|   of it sim-only: exact w ≤ 0.471 (mm) | 6.7 | 7.1 | 3.6 | 5.0 |
|   of it thin layer: w > 0.471, L ≤ 0.471 (mm) | 178.2 | 68.4 | 174.4 | 174.2 |
|   of it still over: L > 0.471 (mm) | 596.0 | 410.8 | 866.1 | 879.5 |
| thin-layer share of the old count | 22.8 % | 14.1 % | 16.7 % | 16.5 % |
| new over: sim radial ≤ 0.471, L > 0.471 (mm) | 2.4 | 0.0 | 0.3 | 0.3 |
| **area count: L > 0.471 (mm)** | **598.4** | **410.8** | **866.3** | **879.8** |
|   still over at a lowered feed / at the base feed (mm) | 2.7 / 593.3 | 5.7 / 405.1 | 400.4 / 465.6 | 400.4 / 479.1 |
|   still over with full-height stock, hmax ≥ d (mm) | 163.8 | 146.2 | 353.7 | 355.1 |
| base clearing feed (mm/min) | 3556 | 4000 | 4000 | 4000 |
| peak L | 2.929 | 2.735 | 3.746 | 3.746 |
| thin layer by hmax: ≤ 0.5 / 0.5–1.5 / 1.5–3 / > 3 mm (mm) | 137.0 / 31.2 / 2.7 / 7.3 | 32.2 / 27.4 / 3.9 / 5.0 | 135.7 / 30.8 / 2.3 / 5.6 | 135.5 / 30.8 / 2.3 / 5.6 |
| thin layer: mean height over the width, A / (w·D) (mm) | 0.64 | 0.69 | 0.61 | 0.61 |
| thin layer that the sim volume form also reads ≤ 0.471 | 91.3 % | 77.2 % | 92.1 % | 92.1 % |

### wanaka200, op 1 "2 Back Rough"

| | D26 (`fix16/wanaka200_pick`) | C35 (`fix12/wanaka200`) | D33 (`fix23/default/wanaka200`) | D33 stock_depth (`fix23/stock_depth/wanaka200`) |
|---|---|---|---|---|
| op 1 Depth/Pass d (mm) | 5.46 | 5.46 | 5.46 | 5.46 |
| old count: sim radial > 0.471 (mm) | 2225.3 | 2234.8 | 2558.4 | 2864.0 |
|   of it sim-only: exact w ≤ 0.471 (mm) | 90.0 | 89.9 | 92.0 | 126.7 |
|   of it thin layer: w > 0.471, L ≤ 0.471 (mm) | 604.2 | 613.9 | 650.2 | 685.2 |
|   of it still over: L > 0.471 (mm) | 1531.1 | 1531.0 | 1816.3 | 2052.1 |
| thin-layer share of the old count | 27.2 % | 27.5 % | 25.4 % | 23.9 % |
| new over: sim radial ≤ 0.471, L > 0.471 (mm) | 0.0 | 0.0 | 0.0 | 0.0 |
| **area count: L > 0.471 (mm)** | **1531.1** | **1531.0** | **1816.3** | **2052.1** |
|   still over at a lowered feed / at the base feed (mm) | 0.0 / 1531.1 | 0.0 / 1531.0 | 37.3 / 1779.0 | 44.1 / 2008.1 |
|   still over with full-height stock, hmax ≥ d (mm) | 52.2 | 52.1 | 150.6 | 143.1 |
| base clearing feed (mm/min) | 1000 | 1000 | 1000 | 1000 |
| peak L | 2.994 | 3.093 | 1.957 | 1.957 |
| thin layer by hmax: ≤ 0.5 / 0.5–1.5 / 1.5–3 / > 3 mm (mm) | 12.6 / 420.0 / 136.1 / 35.5 | 18.5 / 423.8 / 136.1 / 35.5 | 13.8 / 447.7 / 154.9 / 33.9 | 16.5 / 454.9 / 168.0 / 45.8 |
| thin layer: mean height over the width, A / (w·D) (mm) | 0.62 | 0.62 | 0.61 | 0.64 |
| thin layer that the sim volume form also reads ≤ 0.471 | 95.7 % | 95.7 % | 95.0 % | 94.5 % |

## 7. Reading

1. **Most of the old over-load was thin layers.** On Corne the thin-layer
   class is 22.8 % of the old count on D26, 14.1 % on C35 and
   16.7 % on D33. On wanaka200 it is 27.2 %, 27.5 % and
   25.4 %.
2. **The area count.** Corne: D26 598.4 mm, C35
   410.8 mm, D33 866.3 mm. wanaka200:
   1531.1, 1531.0 and 1816.3 mm.
3. **On Corne D33 the rest is the fed slot.** 400.4 mm of
   D33's 866.1 mm still over runs at a lowered feed (the C38
   fed slots and the capped turns); 465.6 mm runs at the
   base feed. A slot is a cut over the full width at full depth, so the
   area rule keeps it over. The judge exempts a capped turn; the lead
   decides if a fed slot gets the same rule.
4. **What the thin class is.** Most of its length has stock higher than
   3 mm on part of the width (hmax > 3 mm: Corne D33 5.6 of
   174.4 mm; wanaka200 D33 33.9 of
   650.2 mm). There the cutter takes a deep cut that is
   narrower than the bound, and a thin layer beside it. The width count
   read that as a cut over the full width. A thin layer across the whole disc (hmax ≤ 1.5 mm)
   is Corne D33 166.5 mm and wanaka200 D33
   461.5 mm.
5. **No deep cut was hidden.** The new-over class (a cut deeper than d
   under a radial of 0.471 or less) is 6.5 mm or less in
   every run.
6. **The sim's volume agrees in the main.** The sim's own area form
   (removed volume / XY step / (D·d)) also reads 0.471 or less on
   77.2 % or more of the Corne thin-layer length and
   94.5 % or more on wanaka200. One sim sample lumps its
   removal (`adaptive/CLAUDE.md`), so this is a cross-check, not the judge.

## 8. Limits and open points

- The formula waits for C22's agreement. If C22 changes it, `area_oracle.py`
  and this README change with it.
- The base feed of D26 Corne is 3556 mm/min, not 4000, so its feed split
  is not comparable with the other columns.
- The oracle stamps a helix chord as a straight step (wanaka200: up to
  1.80 mm).
- The stock outside the stock box is air. The part surface is not read:
  a cut never goes below the target, so the stock above the tip is the
  only quantity.

## 9. Files

- `area_oracle.py`: the oracle. `rescore.py`: the classes (`rescore.json`).
- `synthetic.py`: the synthetic checks (`synthetic.json`).
- `make_readme.py`: this file. `assert_fix24.py`: the checks.
- `run_fix24.sh`: the oracle on the eight runs, then the rest, then the checks.
- `area_<run>.npz`: one row per read sample.
