"""D69: the stock top after each op of a play run, fix26/stockmap_d35.py's method (copy of d68/corne_op5.py `lower`): a flat
disc of the op's tool radius lowered to each sample tip Z on a 0.25 mm grid. Only the cutting samples (not rapids).
Writes stock_<label>.npz with top_after_<tp> for each tp in order. Usage: stockmap.py <run dir> <label> <tp:R,...>"""
import json, math, sys
import numpy as np
run, label, spec = sys.argv[1], sys.argv[2], sys.argv[3]
sim = json.load(open(f"{run}/simulation.json")); T = sim["trace"]["samples"]
lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]; g = 0.25
nx = int(round((hi[0] - lo[0]) / g)); ny = int(round((hi[1] - lo[1]) / g))
def lower(top, P, R):
    seg = np.diff(P, axis=0); L = np.hypot(seg[:, 0], seg[:, 1]); n = np.maximum(1, np.ceil(L / (g / 2)).astype(int))
    idx = np.repeat(np.arange(len(seg)), n); f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
    D = np.vstack([P[:1], P[idx] + f[:, None] * seg[idx]])
    ci = np.floor((D[:, 0] - lo[0]) / g).astype(int); cj = np.floor((D[:, 1] - lo[1]) / g).astype(int); rr = int(math.ceil(R / g)) + 1
    for dj in range(-rr, rr + 1):
        for di in range(-rr, rr + 1):
            I, J = ci + di, cj + dj; cx = lo[0] + (I + 0.5) * g; cy = lo[1] + (J + 0.5) * g
            m = ((cx - D[:, 0]) ** 2 + (cy - D[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
            if m.any(): np.minimum.at(top, (J[m], I[m]), D[m, 2])
top = np.full((ny, nx), hi[2]); out = {"lo": np.array(lo), "g": g}
for item in spec.split(","):
    tp, R = item.split(":"); tp = int(tp); R = float(R)
    # one continuous run per non-rapid stretch, so a rapid never sweeps the disc
    runs, cur, last = [], [], None
    for s in T:
        if s["toolpath_id"] != tp: continue
        if s["cut_kinematics"] == "rapid":
            if len(cur) > 1: runs.append(np.array(cur))
            cur = []; continue
        cur.append(s["position"])
    if len(cur) > 1: runs.append(np.array(cur))
    for P in runs: lower(top, P, R)
    out[f"top_after_{tp}"] = top.copy(); print(label, tp, "runs", len(runs), file=sys.stderr)
np.savez_compressed(f"stock_{label}.npz", **out)
