"""D51: the stock that the wanaka back rough leaves inside the part, per run, from the stock maps of op7_wanaka.py
(fix33/stock_cp_alone_0.6.npz: CP alone 0.6, op 1 only; fix35/stock_wanaka_s2p0.npz and fix34/stock_wanaka_s3p5.npz: geo
s2.0 / s3.5 + op 7 on 453d52571). Grid 0.5 mm; a flat 6 mm disc lowered to each trace sample tip (D47 item 4).
- excess e = stock top - target, target = reach floor + 4.0 (op 1 stock_to_leave_axial). e > 0 is stock over stock_to_leave.
- 'core': inside the part, > 3 mm from the outline (the 0-3 mm row is out of reach of every run: op 1's boundary is
  inside, so CP leaves all of it, D47 item 4).
- Thresholds: e > 0 (stock_to_leave 4.0), e > 0.6 (+ one sim cell, resolution 0.6), e > 0.8 (fix9's rule).
- Slope: the part surface seen from below (7 - top) on the grid, central differences, degrees.
- Per slope bin: core area, mean e and area e > 0.6, after op 1 and after the job.
Writes leftover.json and img/leftover_maps.png."""
import json, math
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent
RUNS = {"CP alone 0.6": P3 / "fix33/stock_cp_alone_0.6.npz", "geo s2.0": H / "stock_wanaka_s2p0.npz", "geo s3.5": P3 / "fix34/stock_wanaka_s3p5.npz"}
D = {k: np.load(v) for k, v in RUNS.items()}
g = float(D["CP alone 0.6"]["g"]); PL = D["CP alone 0.6"]["PL"]; tgt = D["CP alone 0.6"]["target"]
for k, d in D.items():
    assert np.array_equal(d["PL"], PL) and np.allclose(d["target"], tgt), k
inside = PL < 4; core = (PL >= 1) & (PL <= 3)
reach = tgt - 4.0
gy, gx = np.gradient(reach, g); slope = np.degrees(np.arctan(np.hypot(gx, gy)))
cell = g * g
BINS = [0, 5, 10, 20, 30, 45, 90]
out = {"grid_mm": g, "core_mm2": round(float(core.sum() * cell))}
for k, d in D.items():
    r = {}
    for tag, top in (("after op 1", d["before"]), ("after job", d["after"])):
        e = (top - tgt)[core]
        r[tag] = {"max": round(float(e.max()), 2), "p99": round(float(np.percentile(e, 99)), 2), "p95": round(float(np.percentile(e, 95)), 2),
                  "p50": round(float(np.percentile(e, 50)), 2),
                  "area_e>0_mm2": round(float((e > 0.0).sum() * cell)), "area_e>0.6_mm2": round(float((e > 0.6).sum() * cell)),
                  "area_e>0.8_mm2": round(float((e > 0.8).sum() * cell)), "vol_e>0_mm3": round(float(np.clip(e, 0, None).sum() * cell)),
                  "vol_e>0.6_mm3": round(float(np.clip(e - 0.6, 0, None).sum() * cell))}
        sb = {}
        E = top - tgt
        for a, b in zip(BINS, BINS[1:]):
            m = core & (slope >= a) & (slope < b)
            if m.sum() == 0: continue
            sb[f"{a}-{b} deg"] = {"core_mm2": round(float(m.sum() * cell)), "mean_e": round(float(np.clip(E[m], 0, None).mean()), 3),
                                  "p95_e": round(float(np.percentile(E[m], 95)), 2), "area_e>0.6_mm2": round(float(((E > 0.6) & m).sum() * cell))}
        r[tag]["by_slope"] = sb
    out[k] = r
json.dump(out, open(H / "leftover.json", "w"), indent=1)
for k in RUNS:
    for tag in ("after op 1", "after job"):
        r = out[k][tag]; print(f"{k:13s} {tag:10s}", {x: r[x] for x in r if x != "by_slope"})
    for b, v in out[k]["after job"]["by_slope"].items(): print("     ", b, v)
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
(H / "img").mkdir(exist_ok=True)
fig, axs = plt.subplots(1, 4, figsize=(26, 6.8))
ext = [0, PL.shape[1] * g, 0, PL.shape[0] * g]
for ax, (k, d) in zip(axs, D.items()):
    e = np.where(inside, d["after"] - tgt, np.nan)
    im = ax.imshow(e, origin="lower", extent=ext, cmap="magma_r", vmin=0, vmax=3)
    a = out[k]["after job"]
    ax.set_title(f"{k}: excess over stock_to_leave after the job\ncore e>0.6: {a['area_e>0.6_mm2']} mm2, p99 {a['p99']} mm, max {a['max']} mm", fontsize=9)
    ax.set_xlim(18, 222); ax.set_ylim(23, 227); ax.set_aspect("equal")
fig.colorbar(im, ax=axs[:3], label="stock above target (mm), shared scale 0-3", shrink=0.8)
im2 = axs[3].imshow(np.where(inside, slope, np.nan), origin="lower", extent=ext, cmap="viridis", vmin=0, vmax=45)
axs[3].set_title("part surface slope (deg), setup-0 frame"); axs[3].set_xlim(18, 222); axs[3].set_ylim(23, 227); axs[3].set_aspect("equal")
fig.colorbar(im2, ax=axs[3], shrink=0.8)
fig.savefig(H / "img" / "leftover_maps.png", dpi=80, bbox_inches="tight")
