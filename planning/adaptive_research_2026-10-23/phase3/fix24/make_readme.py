"""C43 (fix24): writes README.md from rescore.json, synthetic.json and
judge_section.md (the C12 judge change). assert_fix24.py checks every number."""
import json
from pathlib import Path
H = Path(__file__).resolve().parent
J = json.load(open(H / "rescore.json"))
SYN = json.load(open(H / "synthetic.json"))
JUDGE = open(H / "judge_section.md").read()
ARMS = ["contour_parallel", "D26", "C35", "D33", "D33 stock_depth"]
ROWS = [("op 1 Depth/Pass d (mm)", lambda r: f"{r['dpp']:.2f}"),
        ("old count: sim radial > 0.471 (mm)", lambda r: f"{r['old_over_mm']:.1f}"),
        ("**load count, per strip: L > 0.471 (mm)**", lambda r: f"**{r['area_over_mm']:.1f}**"),
        ("box count, info: L_box > 0.471 (mm)", lambda r: f"{r['box_over_mm']:.1f}"),
        ("box-only: L_box > 0.471, L ≤ 0.471 (mm)", lambda r: f"{r['box_only_mm']:.1f}"),
        ("old count, of it sim-only: L ≤ 0.471, w ≤ 0.471 (mm)", lambda r: f"{r['sim_only_mm']:.1f}"),
        ("old count, of it thin: w > 0.471, L ≤ 0.471 (mm)", lambda r: f"{r['thin_layer_mm']:.1f}"),
        ("old count, of it still over: L > 0.471 (mm)", lambda r: f"{r['still_over_mm']:.1f}"),
        ("new over: sim radial ≤ 0.471, L > 0.471 (mm)", lambda r: f"{r['new_over_mm']:.1f}"),
        ("load count at a lowered feed / at the base feed (mm)", lambda r: f"{r['still_over_reduced_feed_mm']:.1f} / {r['still_over_base_feed_mm']:.1f}"),
        ("base clearing feed (mm/min)", lambda r: f"{r['base_feed']:.0f}"),
        ("peak L", lambda r: f"{r['area_peak_L']:.3f}")]


def table(fx):
    head = "| | " + " | ".join(f"{a} (`{J[f'{fx} {a}']['run']}`)" for a in ARMS) + " |\n|---|" + "---|" * len(ARMS) + "\n"
    return head + "".join(f"| {lab} | " + " | ".join(f(J[f"{fx} {a}"]) for a in ARMS) + " |\n" for lab, f in ROWS)


def syn():
    out = "| stepover e (mm) | stock height t (mm) | heading (rad) | L read | L = e·t/(D·d) | w read | w = e/D |\n|---|---|---|---|---|---|---|\n"
    for c in SYN["cases"]:
        out += f"| {c['e']:.1f} | {c['t']:.1f} | {c['ang']:.1f} | {c['L']:.4f} | {c['L_expect']:.4f} | {c['w']:.4f} | {c['w_expect']:.4f} |\n"
    return out


g = lambda k: J[k]
cp_c, cp_w = g("corne contour_parallel"), g("wanaka200 contour_parallel")
c33, w33 = g("corne D33"), g("wanaka200 D33")
README = f"""# fix24 (C43): the depth-weighted load oracle

Operator ruling, 2026-10-27: the load bound weights the engagement by the
axial depth. Operator instruction, the same day: use the load calculations
that the codebase already has, with one definition across the codebase.

C18 owns the measurement side: the exact oracle, the C12 heavy test judge
and the fixture counts. C22 owns the planner side (`P3_C22_3d_turns.md`
§18.6). Python only on the fixtures; the C12 test edit is in §4.

## 1. The definition (lead's ruling 2026-10-27; C22 §18.6)

The load of a step is the codebase's own chip cross-section, applied per
lateral strip of the cutter disc, over that of a full pass:

`A = Σ over the strips of cutter.mrr_cross_section_mm2(h(u), strip width)`

`L = A / cutter.mrr_cross_section_mm2(d, D)`

- `MillingCutter::mrr_cross_section_mm2` (`crates/rs_cam_core/src/tool/mod.rs:508`)
  is ap · ae. The V-bit halves it (`tool/vbit.rs:149`); the factor cancels
  in the ratio. The power gate multiplies it by the feed for the MRR
  (`tool_load/power.rs:287`).
- u: the lateral coordinate, perpendicular to the XY travel of the step.
  The strips are one lattice pitch wide on the exact oracle (0.05 mm) and
  one planner cell wide on the planner.
- h(u): the deepest H − (z + p(r)) of the disc points in the strip with
  stock above the cutter. H is the stock top, z the tip Z, p(r) the
  cutter's height above its tip at radius r (`height_at_radius`; 0 for the
  flat end mill of every fixture and test). A point has stock above the
  cutter when H − (z + p(r)) > 0.01 mm (the C12 oracle's 0.01).
- d: the Depth/Pass of the op that is read. D: the tool diameter.
- Bound: 0.471, the radial bound of today.
- **Screen only:** the simulation's box `radial_woc_fraction ·
  axial_engagement_mm / d` (the deepest point times the full width). It
  is an upper bound of L: on the exact lattice the box is under L by at
  most two strips, from the rounding of the end strips. A sample is read
  on the exact lattice when the sim radial or the sim box is over the
  screen bound.

**Info column:** `L_box`, the same box on the exact lattice:
mrr_cross_section_mm2(ap, ae) / mrr_cross_section_mm2(d, D), with ap the
deepest uncut height in the disc and ae its lateral extent.

## 2. The derivation (no new number)

- At a full pass h(u) = d across the engaged width, so A = d · ae and
  L = ae / D, the radial fraction of today. The ceiling
  (1 − cos(1.05 · acos(1 − s/R))) / 2 and its tolerance terms carry over
  with no change. That is 0.471 at s 2, R 3.
- A thin layer of height t < d across the band reads L = (ae / D) · t / d:
  light. A narrow deep strip counts only its own width.
- A cut deeper than d reads L > ae / D: held, and held tighter than now.
- The lattice reads A to one strip: |ΔL| ≤ pitch · h / (D · d).
- d is read from the op's project file: Corne op 1 "3D Rough 1" 6.0 mm
  (`corne_job_corrected_c12_agent_search.toml` line 127), wanaka200 op 1
  "2 Back Rough" 5.46 mm (`wanaka200_mt1_c12_rough_only.toml` line 256).

## 3. The oracle: `area_oracle.py`

- The height field H is a lattice of pitch 0.05 mm (the C12
  `EXACT_PITCH_MM`) over the stock box, from the stock top. A point outside
  the box holds no stock. wanaka200: the samples are in the setup frame of
  the flipped stock, so the box is x 0–240, y 0–250, z 0–25
  (`fix9/stockmap.py`).
- The samples are walked in trace order. Each sample is read first, then
  its step (from the previous sample of the same toolpath) is stamped: a
  flat end mill of R 3 mm sets H = min(H, z) at the lattice points within R
  of points 0.3 mm apart or less along the step, Z linear along the step.
  So a read sees every earlier step and the current move up to `from`, as
  `exact_width_at` does. The longest sample step is 0.54 mm on Corne
  and 1.83 mm on wanaka200.
- The read: the disc of R at the step midpoint, the tip Z at the midpoint.
- Usage: `area_oracle.py <run> <corne|wanaka200> [toolpath id] [Depth/Pass]`.
  A later toolpath (a CP rest op) is read on the stock that the earlier
  toolpaths of the trace leave. Every toolpath stamps with R 3 mm, so an
  earlier toolpath with another tool gives a wrong stock.

The synthetic checks (`synthetic.py`): a straight pass, then a parallel
pass at stepover e on stock of height t over the tip (d = 6.0 mm, R 3 mm).

{syn()}
Each L is within one lattice bin, pitch · t / (D · d), of e·t/(D·d), and
each w within pitch / D = 0.0083 of e/D. At t = 9.0 mm > d the load is
{SYN['cases'][6]['L']:.4f} against w {SYN['cases'][6]['w']:.4f}: a deep side cut is held.

{JUDGE}
## 5. The fixture counts

The old count (C22 §16.9; `fix21`–`fix23` `load_split.py`) is the XY length
of op 1's clearing_cut samples with sim radial > 0.471. The new count is
the XY length of the read samples with L > 0.471 (`rescore.py`,
`area_over_mm`). The old counts here are equal to `P15_D_verify.md` (D26,
D33 default, D33 stock_depth), `P3_C38_outer_band.md` §16.4 (C35) and
`P3_C18_3d_oracle.md` §C32.2 (wanaka200 contour_parallel, "body over 0.471").

## 6. The re-score (read only)

The classes of the old count: sim-only (L ≤ 0.471 and the exact width
w ≤ 0.471), thin (w > 0.471, L ≤ 0.471), still over (L > 0.471).
"Lowered feed": a sample feed below the op's base clearing feed, the most
common one (the `load_split.py` rule): the C38 fed slots and the capped
turns.

### Corne, op 1 "3D Rough 1"

{table("corne")}
### wanaka200, op 1 "2 Back Rough"

{table("wanaka200")}
## 7. Reading

1. **Most of the old over-load was thin layers.** The per-strip load count
   against the old count: Corne D26 {g('corne D26')['area_over_mm']:.1f} of {g('corne D26')['old_over_mm']:.1f} mm,
   C35 {g('corne C35')['area_over_mm']:.1f} of {g('corne C35')['old_over_mm']:.1f} mm, D33 {c33['area_over_mm']:.1f} of
   {c33['old_over_mm']:.1f} mm; wanaka200 D26 {g('wanaka200 D26')['area_over_mm']:.1f} of
   {g('wanaka200 D26')['old_over_mm']:.1f} mm, C35 {g('wanaka200 C35')['area_over_mm']:.1f} of {g('wanaka200 C35')['old_over_mm']:.1f} mm,
   D33 {w33['area_over_mm']:.1f} of {w33['old_over_mm']:.1f} mm.
2. **The control.** contour_parallel reads {cp_c['area_over_mm']:.1f} mm on Corne
   and {cp_w['area_over_mm']:.1f} mm on wanaka200. Every research column is
   below it on both parts.
3. **The box would keep most of it.** The box (info) reads
   {c33['box_over_mm']:.1f} mm on Corne D33 and {w33['box_over_mm']:.1f} mm on wanaka200 D33.
   The difference is disc reads with a deep part on a narrow strip and a
   thin layer on the rest: the box multiplies the full width by the deepest
   point. That is why the box is only the screen.
4. **The D33 slots, as context.** {c33['still_over_reduced_feed_mm']:.1f} mm of the Corne D33
   load count runs at a lowered feed (the C38 fed slots and the capped
   turns). The edges move to a separate contour pass (C12 §16), so these
   slots leave the geodesic op.
5. **Deep cuts.** The peak L is {max(g(k)['area_peak_L'] for k in J if 'contour' not in k):.3f} in the research columns
   and {max(cp_c['area_peak_L'], cp_w['area_peak_L']):.3f} in contour_parallel. The new-over class (over under the
   load, not under the sim radial) is {max(J[k]['new_over_mm'] for k in J):.1f} mm or less in every run.

## 8. Limits

- The base feed of D26 Corne is 3556 mm/min, not 4000, so its feed split is
  not comparable with the other columns.
- The oracle stamps each sample step, an arc or helix chord included, as
  a straight step (wanaka200: up to 1.83 mm).
- The stock outside the stock box is air. A cut never goes below the
  target, so the stock above the tip is the only quantity.

## 9. Files

- `area_oracle.py`: the oracle. `rescore.py`: the classes (`rescore.json`).
- `synthetic.py`: the synthetic checks (`synthetic.json`).
- `judge_section.md`: §4. `make_readme.py`: this file. `assert_fix24.py`: the checks.
- `run_fix24.sh`: the oracle on the ten runs, then the rest, then the checks.
- `area_<run>.npz`: one row per read sample.
"""
open(H / "README.md", "w").write(README)
