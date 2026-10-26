"""C43 (fix24): writes README.md from rescore.json and the synthetic checks
(synthetic.py). assert_fix24.py checks every number in it."""
import json
from pathlib import Path
H = Path(__file__).resolve().parent
J = json.load(open(H / "rescore.json"))
SYN = json.load(open(H / "synthetic.json"))
ARMS = ["D26", "C35", "D33", "D33 stock_depth"]


def table(fx):
    rows = [("op 1 Depth/Pass d (mm)", lambda r: f"{r['dpp']:.2f}"),
            ("old count: sim radial > 0.471 (mm)", lambda r: f"{r['old_over_mm']:.1f}"),
            ("  of it sim-only: exact w ≤ 0.471 (mm)", lambda r: f"{r['sim_only_mm']:.1f}"),
            ("  of it thin layer: w > 0.471, L ≤ 0.471 (mm)", lambda r: f"{r['thin_layer_mm']:.1f}"),
            ("  of it still over: L > 0.471 (mm)", lambda r: f"{r['still_over_mm']:.1f}"),
            ("thin-layer share of the old count", lambda r: f"{100 * r['thin_layer_mm'] / r['old_over_mm']:.1f} %"),
            ("new over: sim radial ≤ 0.471, L > 0.471 (mm)", lambda r: f"{r['new_over_mm']:.1f}"),
            ("**area count: L > 0.471 (mm)**", lambda r: f"**{r['area_over_mm']:.1f}**"),
            ("  still over at a lowered feed / at the base feed (mm)", lambda r: f"{r['still_over_reduced_feed_mm']:.1f} / {r['still_over_base_feed_mm']:.1f}"),
            ("  still over with full-height stock, hmax ≥ d (mm)", lambda r: f"{r['still_over_full_depth_mm']:.1f}"),
            ("base clearing feed (mm/min)", lambda r: f"{r['base_feed']:.0f}"),
            ("peak L", lambda r: f"{r['area_peak_L']:.3f}"),
            ("thin layer by hmax: ≤ 0.5 / 0.5–1.5 / 1.5–3 / > 3 mm (mm)", lambda r: " / ".join(f"{v:.1f}" for v in r["thin_by_hmax_mm"])),
            ("thin layer: mean height over the width, A / (w·D) (mm)", lambda r: f"{r['thin_mean_height_mm']:.2f}"),
            ("thin layer that the sim volume form also reads ≤ 0.471", lambda r: f"{100 * r['thin_simvol_agree_frac']:.1f} %")]
    head = "| | " + " | ".join(f"{a} (`{J[f'{fx} {a}']['run']}`)" for a in ARMS) + " |\n|---|" + "---|" * len(ARMS) + "\n"
    return head + "".join(f"| {lab} | " + " | ".join(f(J[f"{fx} {a}"]) for a in ARMS) + " |\n" for lab, f in rows)


def syn():
    out = "| stepover e (mm) | stock height t (mm) | heading (rad) | L read | L = e·t/(D·d) | w read | w = e/D |\n|---|---|---|---|---|---|---|\n"
    for c in SYN["cases"]:
        out += f"| {c['e']:.1f} | {c['t']:.1f} | {c['ang']:.1f} | {c['L']:.4f} | {c['L_expect']:.4f} | {c['w']:.4f} | {c['w_expect']:.4f} |\n"
    return out


c, d = J["corne D33"], J["corne D26"]; c35 = J["corne C35"]; w26, w35, w33 = J["wanaka200 D26"], J["wanaka200 C35"], J["wanaka200 D33"]
pct = lambda r: 100 * r["thin_layer_mm"] / r["old_over_mm"]
README = f"""# fix24 (C43): the depth-weighted load oracle

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

{syn()}
Each L is within one bin, pitch · t / (D · d), of e·t/(D·d), and each w
is within pitch / D = 0.0083 of e/D. At t = 9.0 mm > d the load is
{SYN['cases'][6]['L']:.4f} against w {SYN['cases'][6]['w']:.4f}: a deep side cut is held.

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
) -> f64 {{
    let r = TOOL_RADIUS;
    let pitch = EXACT_PITCH_MM;
    let half = 2.0 * r;
    let n = (2.0 * half / pitch) as usize + 1;
    let (x0, y0) = (to.0 - half, to.1 - half);
    let mut top = vec![TOP_Z; n * n];
    let stamp = |top: &mut Vec<f64>, px: f64, py: f64, pz: f64| {{
        // the loop of `exact_width_at`'s stamp, with
        // `top[j * n + i] = top[j * n + i].min(pz)` in place of `cut[..] = true`
    }};
    // every earlier move in full, then move `m` up to `from`, with
    // `move_points` extended to give the Z of each point (linear along the move);
    // `tip` is the mean of the Z at `from` and at `to`
    // ...
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l = dx.hypot(dy).max(1e-9);
    let (sx, sy) = (-dy / l, dx / l);
    let mid = (0.5 * (from.0 + to.0), 0.5 * (from.1 + to.1));
    let mut h = vec![0.0_f64; (2.0 * r / pitch).round() as usize + 2];
    for j in 0..n {{
        for i in 0..n {{
            let (x, y) = (x0 + i as f64 * pitch, y0 + j as f64 * pitch);
            let above = top[j * n + i] - tip;
            if above > 0.01 && (x - mid.0).hypot(y - mid.1) <= r {{
                let u = (x - mid.0) * sx + (y - mid.1) * sy;
                let b = ((u + r) / pitch).round() as usize;
                h[b] = h[b].max(above);
            }}
        }}
    }}
    h.iter().sum::<f64>() * pitch / (2.0 * r * depth_per_pass)
}}
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

{table("corne")}
### wanaka200, op 1 "2 Back Rough"

{table("wanaka200")}
## 7. Reading

1. **Most of the old over-load was thin layers.** On Corne the thin-layer
   class is {pct(d):.1f} % of the old count on D26, {pct(c35):.1f} % on C35 and
   {pct(c):.1f} % on D33. On wanaka200 it is {pct(w26):.1f} %, {pct(w35):.1f} % and
   {pct(w33):.1f} %.
2. **The area count.** Corne: D26 {d['area_over_mm']:.1f} mm, C35
   {c35['area_over_mm']:.1f} mm, D33 {c['area_over_mm']:.1f} mm. wanaka200:
   {w26['area_over_mm']:.1f}, {w35['area_over_mm']:.1f} and {w33['area_over_mm']:.1f} mm.
3. **On Corne D33 the rest is the fed slot.** {c['still_over_reduced_feed_mm']:.1f} mm of
   D33's {c['still_over_mm']:.1f} mm still over runs at a lowered feed (the C38
   fed slots and the capped turns); {c['still_over_base_feed_mm']:.1f} mm runs at the
   base feed. A slot is a cut over the full width at full depth, so the
   area rule keeps it over. The judge exempts a capped turn; the lead
   decides if a fed slot gets the same rule.
4. **What the thin class is.** Most of its length has stock higher than
   3 mm on part of the width (hmax > 3 mm: Corne D33 {c['thin_by_hmax_mm'][3]:.1f} of
   {c['thin_layer_mm']:.1f} mm; wanaka200 D33 {w33['thin_by_hmax_mm'][3]:.1f} of
   {w33['thin_layer_mm']:.1f} mm). There the cutter takes a deep cut that is
   narrower than the bound, and a thin layer beside it. The width count
   read that as a cut over the full width. A thin layer across the whole disc (hmax ≤ 1.5 mm)
   is Corne D33 {c['thin_by_hmax_mm'][0] + c['thin_by_hmax_mm'][1]:.1f} mm and wanaka200 D33
   {w33['thin_by_hmax_mm'][0] + w33['thin_by_hmax_mm'][1]:.1f} mm.
5. **No deep cut was hidden.** The new-over class (a cut deeper than d
   under a radial of 0.471 or less) is {max(J[k]['new_over_mm'] for k in J):.1f} mm or less in
   every run.
6. **The sim's volume agrees in the main.** The sim's own area form
   (removed volume / XY step / (D·d)) also reads 0.471 or less on
   {100 * min(J[k]['thin_simvol_agree_frac'] for k in J if k.startswith('corne')):.1f} % or more of the Corne thin-layer length and
   {100 * min(J[k]['thin_simvol_agree_frac'] for k in J if k.startswith('wanaka')):.1f} % or more on wanaka200. One sim sample lumps its
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
"""
open(H / "README.md", "w").write(README)
