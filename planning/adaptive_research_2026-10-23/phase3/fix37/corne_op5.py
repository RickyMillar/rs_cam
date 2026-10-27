"""D54: a copy of fix34/corne_op5.py (writes into fix37). D49: a copy of fix31/op5_analysis.py (writes into fix34). Added: the reach measure. Op 1 boundary = model_silhouette,
containment center, offset 6, so op 1's tool centres stay within 6 mm of the silhouette and its reach ends 9 mm outside it:
the volume and area that op 5 lowers > 9 mm outside (target 0), and op 5's cutting tool centres > 6 mm outside. D45: a copy of fix29/op5_analysis.py (writes into fix31). D41: where op 5 ("3D Rough 1 (edges, CP rest)", contour_parallel on
from_remaining_stock, the whole silhouette + 6 mm boundary) spends its time.
Per run (the runs):
- the stock before op 5 (toolpath 1 only) and after it (toolpaths 1 and 5),
  fix26/stockmap_d35.py's method (flat 6 mm disc lowered to each sample tip
  Z on a 0.25 mm grid; target = the 6 mm closing of the part + 0.5);
- 'cleared by op 5': cells whose top op 5 lowers by more than 0.8 mm;
- op 5's cutting samples (is_cutting, clearing_cut): time and XY length by
  place (outside the wall 0-3 / 3-6.2 / 6.2-9 / > 9 mm, switch hole, inside the
  case), split by (a) the sample removes stock (> 0.027 mm3, fix11/audit.py's
  quantum) and (b) the stock before op 5 at the sample XY stands > 0.8 mm
  above the target (copy_ramps.py's G-AIRRAMP rule);
- a plot of op 5's samples by class.
- D45: the stock after the copy op (toolpath 3, the 3 mm tool, R 1.5) against
  its own reach target (the closing with R 1.5, + 0.5): left_after_copy_mm2.
Usage: op5_analysis.py <run under phase3> <tag>. Writes op5_<tag>.json, img/op5_<tag>.png."""
import json, math, sys, collections
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
run, tag = sys.argv[1], sys.argv[2]
sim = json.load(open(P3 / run / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]
T = sim["trace"]["samples"]
lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]; g = 0.25; R = 3.0
nx = int(round((hi[0] - lo[0]) / g)); ny = int(round((hi[1] - lo[1]) / g))
xs = lo[0] + (np.arange(nx) + 0.5) * g; ys = lo[1] + (np.arange(ny) + 0.5) * g
def lower(top, P, R=R):
    seg = np.diff(P, axis=0); L = np.hypot(seg[:, 0], seg[:, 1]); n = np.maximum(1, np.ceil(L / (g / 2)).astype(int))
    idx = np.repeat(np.arange(len(seg)), n); f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
    D = np.vstack([P[:1], P[idx] + f[:, None] * seg[idx]])
    ci = np.floor((D[:, 0] - lo[0]) / g).astype(int); cj = np.floor((D[:, 1] - lo[1]) / g).astype(int); rr = int(math.ceil(R / g)) + 1
    for dj in range(-rr, rr + 1):
        for di in range(-rr, rr + 1):
            I, J = ci + di, cj + dj; cx = lo[0] + (I + 0.5) * g; cy = lo[1] + (J + 0.5) * g
            m = ((cx - D[:, 0]) ** 2 + (cy - D[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
            if m.any(): np.minimum.at(top, (J[m], I[m]), D[m, 2])
P1 = np.array([s["position"] for s in T if s["toolpath_id"] == 1]); P5 = np.array([s["position"] for s in T if s["toolpath_id"] == 5])
before = np.full((ny, nx), float(hi[2])); lower(before, P1); after = before.copy(); lower(after, P5)
P3c = np.array([s["position"] for s in T if s["toolpath_id"] == 3]); after3 = after.copy()
if len(P3c) > 1: lower(after3, P3c, 1.5)
part = np.load(P3 / "fix9" / "corne_part.npz"); ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
XX, YY = np.meshgrid(xs, ys)
pi_ = np.round((XX - px0) / pg).astype(int); pj = np.round((YY - py0) / pg).astype(int)
ok = (pi_ >= 0) & (pi_ < ptop.shape[1]) & (pj >= 0) & (pj < ptop.shape[0])
surf = np.zeros(XX.shape); t = np.full(XX.shape, -np.inf); t[ok] = ptop[pj[ok], pi_[ok]]; surf[np.isfinite(t)] = t[np.isfinite(t)]
k = int(math.ceil(R / g)); yy, xx = np.mgrid[-k:k + 1, -k:k + 1]; disc = (xx * g) ** 2 + (yy * g) ** 2 <= R * R
target = ndimage.grey_erosion(ndimage.grey_dilation(surf, footprint=disc, mode="nearest"), footprint=disc, mode="nearest") + 0.5
# D45: the copy op's reach (the 3 mm tool): the same closing with R 1.5
k3 = int(math.ceil(1.5 / g)); y3, x3 = np.mgrid[-k3:k3 + 1, -k3:k3 + 1]; disc3 = (x3 * g) ** 2 + (y3 * g) ** 2 <= 1.5 * 1.5
target3 = ndimage.grey_erosion(ndimage.grey_dilation(surf, footprint=disc3, mode="nearest"), footprint=disc3, mode="nearest") + 0.5
# places on the part grid
fin = np.isfinite(ptop) & (ptop > 1e-6); fill = ndimage.binary_fill_holes(fin); holes = fill & ~fin
lab, n = ndimage.label(holes); area = ndimage.sum(holes, lab, range(1, n + 1)) * pg * pg
keep = np.isin(lab, [i + 1 for i in range(n) if area[i] >= 20.0]); inhole = ndimage.distance_transform_edt(~keep) * pg <= 1.0
dout = ndimage.distance_transform_edt(~fill) * pg
def place_ij(c, r):
    if not (0 <= r < ptop.shape[0] and 0 <= c < ptop.shape[1]): return "outside > 9"
    if fill[r, c]: return "switch hole" if inhole[r, c] else "inside case"
    d = dout[r, c]
    return "outside 0-3" if d <= 3 else "outside 3-6.2" if d <= 6.2 else "outside 6.2-9" if d <= 9 else "outside > 9"
PL = np.vectorize(lambda c, r: place_ij(c, r))(pi_, pj)
cell = g * g
cleared = (before - after) > 0.8
left_before = (before - target) > 0.8
out = {"run": run, "edge_band_0_9_mm2": round(float(((PL == "outside 0-3") | (PL == "outside 3-6.2") | (PL == "outside 6.2-9")).sum() * cell)),
       "cleared_by_op5_mm2": {}, "left_before_op5_mm2": {}, "left_after_op5_mm2": {}}
for pl in ("outside 0-3", "outside 3-6.2", "outside 6.2-9", "outside > 9", "switch hole", "inside case"):
    m = PL == pl
    out["cleared_by_op5_mm2"][pl] = round(float((cleared & m).sum() * cell))
    out["left_before_op5_mm2"][pl] = round(float((left_before & m).sum() * cell))
    out["left_after_op5_mm2"][pl] = round(float((((after - target) > 0.8) & m).sum() * cell))
    out.setdefault("left_after_copy_mm2", {})[pl] = round(float((((after3 - target3) > 0.8) & m).sum() * cell))
cellv = g * g
out["op5_removed_mm3_by_place"] = {pl: round(float(((before - after) * (PL == pl)).sum() * cellv)) for pl in ("outside 0-3", "outside 3-6.2", "outside 6.2-9", "outside > 9", "switch hole", "inside case")}
_C = np.array([x["position"] for x in T if x["toolpath_id"] == 5 and x["is_cutting"]])
_ci = np.round((_C[:, 0] - px0) / pg).astype(int); _cj = np.round((_C[:, 1] - py0) / pg).astype(int)
_ok = (_ci >= 0) & (_ci < ptop.shape[1]) & (_cj >= 0) & (_cj < ptop.shape[0])
_d = np.full(len(_C), 99.0); _d[_ok] = dout[_cj[_ok], _ci[_ok]]
out["op5_cut_centres"] = int(len(_C)); out["centres_beyond_6mm"] = int((_d > 6.0 + 0.3).sum()); out["centre_out_max_mm"] = round(float(_d.max()), 2)
# op 5 samples
S5 = [s for s in T if s["toolpath_id"] == 5]
acc = collections.defaultdict(lambda: collections.Counter())
prev = None; pts = []
for s in S5:
    p = s["position"]; si = s["source_intent"] if isinstance(s["source_intent"], str) else ""
    step = math.hypot(p[0] - prev[0], p[1] - prev[1]) if prev is not None else 0.0; prev = p
    if s["cut_kinematics"] == "rapid": cls = "rapid"
    elif si == "entry_plunge" or (si == "linking" and s["cut_kinematics"] == "plunge"): cls = "plunge/ramp"
    elif si == "clearing_cut":
        rem = s["removed_volume_est_mm3"] > Q
        c = int(np.floor((p[0] - lo[0]) / g)); r = int(np.floor((p[1] - lo[1]) / g))
        lb = left_before[min(max(r, 0), ny - 1), min(max(c, 0), nx - 1)]
        cls = "cut, removes" if rem else ("cut, air, stock > 0.8 before" if lb else "cut, air, stock <= 0.8 before")
    elif si == "linking": cls = "link at feed"
    else: cls = "other"
    c2 = int(round((p[0] - px0) / pg)); r2 = int(round((p[1] - py0) / pg)); pl = place_ij(c2, r2)
    acc[(cls, pl)]["s"] += s["segment_time_s"]; acc[(cls, pl)]["mm"] += step
    pts.append((p[0], p[1], cls))
by_cls = collections.Counter(); by_cls_mm = collections.Counter()
for (cls, pl), v in acc.items(): by_cls[cls] += v["s"]; by_cls_mm[cls] += v["mm"]
out["time_by_class_s"] = {k: round(v, 1) for k, v in by_cls.most_common()}
out["xy_mm_by_class"] = {k: round(v, 1) for k, v in by_cls_mm.most_common()}
out["cut_time_by_place_s"] = {}
for (cls, pl), v in sorted(acc.items(), key=lambda kv: -kv[1]["s"]):
    if cls.startswith("cut"): out["cut_time_by_place_s"][f"{cls} | {pl}"] = round(v["s"], 1)
print(json.dumps(out, indent=1))
json.dump(out, open(H / f"op5_{tag}.json", "w"), indent=1)
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
(H / "img").mkdir(exist_ok=True)
fig, ax = plt.subplots(figsize=(12, 8))
ax.imshow(np.where(fin, ptop, np.nan), origin="lower", extent=[px0, px0 + ptop.shape[1] * pg, py0, py0 + ptop.shape[0] * pg], cmap="Greys", alpha=0.4)
C = {"cut, removes": "tab:red", "cut, air, stock > 0.8 before": "tab:orange", "cut, air, stock <= 0.8 before": "tab:blue", "link at feed": "tab:green", "plunge/ramp": "k", "rapid": "lightgrey", "other": "m"}
pa = np.array([(x, y) for x, y, _ in pts]); cl = np.array([c for _, _, c in pts])
for c in ("rapid", "cut, air, stock <= 0.8 before", "cut, air, stock > 0.8 before", "link at feed", "cut, removes", "plunge/ramp"):
    m = cl == c
    if m.any(): ax.scatter(pa[m, 0], pa[m, 1], s=1, c=C[c], label=f"{c} ({out['time_by_class_s'].get(c, 0)} s)")
ax.set_aspect("equal"); ax.legend(markerscale=8, fontsize=8, loc="lower right"); ax.set_title(f"{tag}: op 5 trace samples by class")
fig.savefig(H / "img" / f"op5_{tag}.png", dpi=100, bbox_inches="tight")
