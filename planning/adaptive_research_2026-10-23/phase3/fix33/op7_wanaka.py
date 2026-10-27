"""D47: the wanaka edge op (toolpath 7, "2 Back Rough [edges, CP rest]") and the wanaka band. A wanaka version of fix31/op5_analysis.py.
The band method (new; the D41 band was Corne-only):
- Frame: the trace samples are in the setup-0 frame of the flipped stock (fix9/stockmap.py): x 0-240, y 0-250, z 0-25 from the
  stock bottom. The part (fix9/wanaka_part.npz, the terrain 200 x 200 mm) maps by X = x - 20, Y = 225 - y; its surface seen from
  below is 7 - top. The part outline is the edge of the finite cells of wanaka_part.npz (the square x 20-220, y 25-225).
- Stock: a flat 6 mm disc lowered to each sample tip Z on a 0.5 mm grid (fix9's grid for wanaka), from the stock top.
- Target: the morphological closing of the part surface by the 3 mm disc (the floor that the tool reaches) + 4.0 (op 1's
  stock_to_leave_axial; fix9's wanaka leave). Outside the outline the surface is 0 (the stock bottom).
- Places, by distance to the outline: inside 0-3 / 3-6.2 / 6.2-9 / > 9 mm, outside 0-3 / > 3 mm. Both ops have the whole
  part as their boundary (op 1: model_silhouette, inside), so outside the outline no op must cut: 'outside' rows show only
  whether a cut goes there ('lowered').
- 'Left': cells whose stock stands > 0.8 mm above the target (fix9's rule). The band is the inside 0-9 mm rows.
Time by class: as op5_analysis.py (removes: removed_volume_est_mm3 > cell^2 * step; air with stock > 0.8 before or not).
Usage: op7_wanaka.py <run under phase3> <tag> [tp of the edge op, default 7; 0 = none (a CP-alone control)]."""
import json, math, sys, collections
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
run, tag = sys.argv[1], sys.argv[2]; TP = int(sys.argv[3]) if len(sys.argv) > 3 else 7
sim = json.load(open(P3 / run / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]
T = sim["trace"]["samples"]
lo0, hi0 = sim["stock_bbox_min"], sim["stock_bbox_max"]; lo = [0.0, 0.0, 0.0]; hi = [hi0[k] - lo0[k] for k in range(3)]
g = 0.5; R = 3.0
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
P1 = np.array([s["position"] for s in T if s["toolpath_id"] == 1])
before = np.full((ny, nx), float(hi[2])); lower(before, P1); after = before.copy()
PT = np.array([s["position"] for s in T if s["toolpath_id"] == TP]) if TP else np.zeros((0, 3))
if len(PT) > 1: lower(after, PT)
part = np.load(P3 / "fix9" / "wanaka_part.npz"); ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
XX, YY = np.meshgrid(xs, ys)
pi_ = np.round((XX - 20.0 - px0) / pg).astype(int); pj = np.round((225.0 - YY - py0) / pg).astype(int)
ok = (pi_ >= 0) & (pi_ < ptop.shape[1]) & (pj >= 0) & (pj < ptop.shape[0])
t = np.full(XX.shape, -np.inf); t[ok] = ptop[pj[ok], pi_[ok]]; inside = np.isfinite(t)
surf = np.zeros(XX.shape); surf[inside] = 7.0 - t[inside]
k = int(math.ceil(R / g)); yy, xx = np.mgrid[-k:k + 1, -k:k + 1]; disc = (xx * g) ** 2 + (yy * g) ** 2 <= R * R
target = ndimage.grey_erosion(ndimage.grey_dilation(surf, footprint=disc, mode="nearest"), footprint=disc, mode="nearest") + 4.0
din = ndimage.distance_transform_edt(inside) * g; dout = ndimage.distance_transform_edt(~inside) * g
PLACES = ("inside 0-3", "inside 3-6.2", "inside 6.2-9", "inside > 9", "outside 0-3", "outside > 3")
PL = np.where(inside, np.where(din <= 3, 0, np.where(din <= 6.2, 1, np.where(din <= 9, 2, 3))), np.where(dout <= 3, 4, 5))
cell = g * g
cleared = (before - after) > 0.8; lb = (before - target) > 0.8; la = (after - target) > 0.8
out = {"run": run, "tp": TP, "grid_mm": g, "inside_mm2": round(float(inside.sum() * cell)), "places_mm2": {}, "lowered_by_op1_mm2": {},
       "cleared_by_edge_op_mm2": {}, "left_before_edge_op_mm2": {}, "left_after_edge_op_mm2": {}}
for i, pl in enumerate(PLACES):
    m = PL == i
    out["places_mm2"][pl] = round(float(m.sum() * cell))
    out["lowered_by_op1_mm2"][pl] = round(float(((hi[2] - before) > 0.8)[m].sum() * cell))
    out["cleared_by_edge_op_mm2"][pl] = round(float((cleared & m).sum() * cell))
    out["left_before_edge_op_mm2"][pl] = round(float((lb & m).sum() * cell))
    out["left_after_edge_op_mm2"][pl] = round(float((la & m).sum() * cell))
out["band_inside_0_9_left_before_mm2"] = sum(out["left_before_edge_op_mm2"][p] for p in PLACES[:3])
out["band_inside_0_9_left_after_mm2"] = sum(out["left_after_edge_op_mm2"][p] for p in PLACES[:3])
out["left_after_max_mm"] = round(float((after - target)[inside].max()), 2)
out["left_after_vol_above_target_inside_mm3"] = round(float(np.clip(after - target, 0, None)[inside].sum() * cell))
np.savez_compressed(H / f"stock_{tag}.npz", before=before, after=after, target=target, PL=PL, g=g)
def place_xy(x, y):
    c = min(max(int(np.floor(x / g)), 0), nx - 1); r = min(max(int(np.floor(y / g)), 0), ny - 1); return PLACES[PL[r, c]], r, c
acc = collections.defaultdict(lambda: collections.Counter()); pts = []; nrapid = 0; prev_rapid = False
for s in (x for x in T if TP and x["toolpath_id"] == TP):
    p = s["position"]; si = s["source_intent"] if isinstance(s["source_intent"], str) else ""
    step = math.hypot(p[0] - pts[-1][0], p[1] - pts[-1][1]) if pts else 0.0
    pl, r, c = place_xy(p[0], p[1])
    if s["cut_kinematics"] == "rapid": cls = "rapid"; nrapid += (not prev_rapid)
    elif si == "entry_plunge" or (si == "linking" and s["cut_kinematics"] == "plunge"): cls = "plunge/ramp"
    elif si == "clearing_cut":
        cls = "cut, removes" if s["removed_volume_est_mm3"] > Q else ("cut, air, stock > 0.8 before" if lb[r, c] else "cut, air, stock <= 0.8 before")
    elif si == "linking": cls = "link at feed"
    else: cls = "other"
    prev_rapid = cls == "rapid"
    acc[(cls, pl)]["s"] += s["segment_time_s"]; acc[(cls, pl)]["mm"] += step; pts.append((p[0], p[1], cls))
by_cls = collections.Counter(); by_mm = collections.Counter()
for (cls, pl), v in acc.items(): by_cls[cls] += v["s"]; by_mm[cls] += v["mm"]
tot = sum(by_cls.values())
out["samples_total_s"] = round(tot, 1); out["time_by_class_s"] = {k: round(v, 1) for k, v in by_cls.most_common()}
out["xy_mm_by_class"] = {k: round(v, 1) for k, v in by_mm.most_common()}
air = sum(v for k, v in by_cls.items() if k.startswith("cut, air")); cut = sum(v for k, v in by_cls.items() if k.startswith("cut"))
out["air_share_of_cut_time"] = round(air / cut, 3) if cut else None
out["rapid_runs_in_trace"] = nrapid
out["cut_time_by_place_s"] = {f"{cls} | {pl}": round(v["s"], 1) for (cls, pl), v in sorted(acc.items(), key=lambda kv: -kv[1]["s"]) if cls.startswith("cut")}
print(json.dumps(out, indent=1))
json.dump(out, open(H / f"op7_{tag}.json", "w"), indent=1)
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
(H / "img").mkdir(exist_ok=True)
fig, axs = plt.subplots(1, 2, figsize=(18, 8))
ax = axs[0]
ax.plot([20, 220, 220, 20, 20], [25, 25, 225, 225, 25], "k-", lw=0.8)
for d in (9.0,):
    ax.plot([20 + d, 220 - d, 220 - d, 20 + d, 20 + d], [25 + d, 25 + d, 225 - d, 225 - d, 25 + d], "k:", lw=0.6)
C = {"cut, removes": "tab:red", "cut, air, stock > 0.8 before": "tab:orange", "cut, air, stock <= 0.8 before": "tab:blue", "link at feed": "tab:green", "plunge/ramp": "k", "rapid": "lightgrey", "other": "m"}
if pts:
    pa = np.array([(x, y) for x, y, _ in pts]); cl = np.array([c for _, _, c in pts])
    for c in ("rapid", "cut, air, stock <= 0.8 before", "cut, air, stock > 0.8 before", "link at feed", "cut, removes", "plunge/ramp"):
        m = cl == c
        if m.any(): ax.scatter(pa[m, 0], pa[m, 1], s=1, c=C[c], label=f"{c} ({out['time_by_class_s'].get(c, 0)} s)")
    ax.legend(markerscale=8, fontsize=8, loc="lower right")
ax.set_aspect("equal"); ax.set_xlim(0, hi[0]); ax.set_ylim(0, hi[1]); ax.set_title(f"{tag}: edge op (tp {TP}) samples by class; solid = part outline, dotted = 9 mm inside")
ax = axs[1]
im = ax.imshow(np.where(inside | (dout <= 3), np.clip(after - target, -1, 6), np.nan), origin="lower", extent=[0, hi[0], 0, hi[1]], cmap="magma_r", vmin=-1, vmax=6)
ax.plot([20, 220, 220, 20, 20], [25, 25, 225, 225, 25], "c-", lw=0.6); fig.colorbar(im, ax=ax, label="stock above target after the edge op (mm)")
ax.set_aspect("equal"); ax.set_title(f"{tag}: left after the edge op (band 0-9 mm inside: {out['band_inside_0_9_left_after_mm2']} mm2)")
fig.savefig(H / "img" / f"op7_{tag}.png", dpi=90, bbox_inches="tight")
